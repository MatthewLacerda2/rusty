//! Automatic exposure (#1444): eye adaptation metered from the view's own HDR
//! scene, Unity HDRP's *Automatic Histogram*.
//!
//! Each frame a view's volume is in `Automatic` mode, two compute passes
//! (`postfx/exposure.wgsl`) build a 64-bin log-luminance histogram of a 64 × 64
//! sample of the scene, average the bins inside the volume's percentile window and
//! ease the **adapted EV** towards the exposure that brings that average to middle
//! grey, within the volume's limits. The adapted EV lives in a 16-byte buffer the
//! composite reads as binding 8 ([`ExposureMeter::state`]), so the frame never waits
//! on the CPU. A copy of it is read back asynchronously, a frame or so late, for
//! `Graphics.GetExposure` ([`ExposureMeter::readout`]).
//!
//! Adaptation runs on game time, snapping outside Play and on a cut
//! ([`params::Clock`]). Each `PostFx`, so each view, owns its meter: the window and
//! a render texture never share an exposure. The pipelines ([`ExposurePasses`]) are
//! the renderer's, built once for every view.

mod effect;
mod params;

pub(crate) use effect::EFFECT;
pub use params::{Clock, ExposureParams, Step};

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::motion::HistoryKey;

/// Histogram bins. Mirrors `BINS` in `postfx/exposure.wgsl`, and equals the
/// histogram pass's workgroup size (8 × 8), one bin per invocation.
const BINS: u64 = 64;
/// Samples per side of the metered grid. Mirrors `GRID`.
const GRID: u32 = 64;
/// The histogram pass's workgroup edge.
const GROUP: u32 = 8;
/// The adapted state's size: `vec4<f32>` (EV, target, held, unused).
const STATE_BYTES: u64 = 16;

/// The meter's two compute pipelines and their one layout: built once, on the
/// `Renderer`, and shared by every view's [`ExposureMeter`].
pub struct ExposurePasses {
    layout: wgpu::BindGroupLayout,
    histogram: wgpu::ComputePipeline,
    reduce: wgpu::ComputePipeline,
}

impl ExposurePasses {
    pub fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let module = registry.load(device, "postfx/exposure.wgsl", "Auto Exposure");
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Auto Exposure"),
            entries: &[
                crate::render::gpu::layout_entries::uniform_entry(0, wgpu::ShaderStages::COMPUTE),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                storage(2),
                storage(3),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Auto Exposure"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Auto Exposure"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        Self {
            histogram: pipeline("cs_histogram"),
            reduce: pipeline("cs_reduce"),
            layout,
        }
    }
}

/// The readback's state, set by the map callback from wgpu's thread.
const IDLE: u8 = 0;
const MAPPING: u8 = 1;
const MAPPED: u8 = 2;

/// One view's meter: its params, histogram, adapted state, clock and readback.
pub struct ExposureMeter {
    params: wgpu::Buffer,
    histogram: wgpu::Buffer,
    state: wgpu::Buffer,
    staging: wgpu::Buffer,
    mapping: Arc<AtomicU8>,
    /// The read in flight began before a cut: its value is dropped, not taken.
    stale: bool,
    readout: Option<f32>,
    /// When the view last adapted; the passes are told how far to move from it.
    clock: Clock,
}

impl ExposureMeter {
    pub fn new(device: &wgpu::Device) -> Self {
        use wgpu::BufferUsages as U;
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let params_size = std::mem::size_of::<ExposureParams>() as u64;
        Self {
            params: buffer(
                "Auto Exposure Params",
                params_size,
                U::UNIFORM | U::COPY_DST,
            ),
            // Zeroed at creation: the first histogram starts empty.
            histogram: buffer("Auto Exposure Histogram", BINS * 4, U::STORAGE),
            state: buffer(
                "Auto Exposure State",
                STATE_BYTES,
                U::STORAGE | U::UNIFORM | U::COPY_SRC,
            ),
            staging: buffer(
                "Auto Exposure Readback",
                STATE_BYTES,
                U::MAP_READ | U::COPY_DST,
            ),
            mapping: Arc::new(AtomicU8::new(IDLE)),
            stale: false,
            readout: None,
            clock: Clock::default(),
        }
    }

    /// How far this frame adapts ([`Clock::step`]). A cut also forgets the readout.
    pub(crate) fn step(&mut self, key: HistoryKey, editor: bool, time: f32) -> Step {
        let step = self.clock.step(key, editor, time);
        if step.cut {
            self.forget();
        }
        step
    }

    /// Cut the clock (a resize, a frame the view did not meter): the next metered
    /// frame snaps, and nothing read back before it is reported.
    pub fn cut(&mut self) {
        self.clock.cut();
        self.forget();
    }

    /// Drop the readout, and the read in flight when it lands.
    fn forget(&mut self) {
        self.readout = None;
        self.stale = self.mapping.load(Ordering::Acquire) != IDLE;
    }

    /// The adapted state the composite reads (binding 8): `x` is the EV.
    pub fn state(&self) -> &wgpu::Buffer {
        &self.state
    }

    /// The adapted EV as last read back, a frame or so behind the GPU; `None` until
    /// a metered frame's copy has landed.
    pub fn readout(&self) -> Option<f32> {
        self.readout
    }

    /// Meter `scene` (the view's HDR target) and adapt, with `params`, then start
    /// reading the result back when no earlier read is in flight.
    pub fn run(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &ExposurePasses,
        scene: &wgpu::TextureView,
        params: &ExposureParams,
    ) {
        self.collect();
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(params));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Auto Exposure"),
            layout: &passes.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scene),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.histogram.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.state.as_entire_binding(),
                },
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Auto Exposure"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Auto Exposure"),
                timestamp_writes: None,
            });
            pass.set_bind_group(0, &bind_group, &[]);
            pass.set_pipeline(&passes.histogram);
            pass.dispatch_workgroups(GRID / GROUP, GRID / GROUP, 1);
            pass.set_pipeline(&passes.reduce);
            pass.dispatch_workgroups(1, 1, 1);
        }
        let read = self.mapping.load(Ordering::Acquire) == IDLE;
        if read {
            encoder.copy_buffer_to_buffer(&self.state, 0, &self.staging, 0, Some(STATE_BYTES));
        }
        queue.submit(std::iter::once(encoder.finish()));
        if read {
            self.mapping.store(MAPPING, Ordering::Release);
            let mapping = Arc::clone(&self.mapping);
            self.staging.map_async(wgpu::MapMode::Read, .., move |res| {
                mapping.store(if res.is_ok() { MAPPED } else { IDLE }, Ordering::Release);
            });
        }
    }

    /// Take a finished readback into [`ExposureMeter::readout`], never waiting.
    fn collect(&mut self) {
        if self.mapping.load(Ordering::Acquire) != MAPPED {
            return;
        }
        if let Ok(view) = self.staging.slice(..).get_mapped_range() {
            let state: &[f32] = bytemuck::cast_slice(&view);
            if !self.stale {
                self.readout = Some(state[0]);
            }
        }
        self.stale = false;
        self.staging.unmap();
        self.mapping.store(IDLE, Ordering::Release);
    }

    /// Wait for the read in flight and take it: a test's or a capture's exact
    /// readout, never the frame's path.
    pub(crate) fn settle(&mut self, device: &wgpu::Device) {
        if self.mapping.load(Ordering::Acquire) == MAPPING {
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
        }
        self.collect();
    }
}

/// The view's side of the readout: what a frontend publishes.
impl crate::render::RenderView {
    /// The automatic exposure this view last read back from the GPU, in EV (#1444):
    /// `None` until an `Automatic` frame's readback has landed. A frontend publishes
    /// the screen's as `Scene::screen_exposure`.
    pub fn exposure_readout(&self) -> Option<f32> {
        self.post_fx.exposure.readout()
    }

    /// [`Self::exposure_readout`] after waiting for the read in flight: exact
    /// for the frame just drawn, for a headless capture that has already waited.
    pub fn settled_exposure(&mut self, device: &wgpu::Device) -> Option<f32> {
        self.post_fx.exposure.settle(device);
        self.exposure_readout()
    }
}

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod stack_gpu_tests;
