mod effect;
mod params;

use crate::gpu::commands::Commands;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
pub(crate) use effect::EFFECT;
pub use params::{Clock, ExposureParams, Step};

use crate::gpu::shaders::ShaderRegistry;
use crate::motion::HistoryKey;
use crate::timing::{GpuPass, GpuTimer};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Mirrors `BINS` in `postfx/exposure.wgsl`: one bin per invocation of an 8 × 8 group.
const BINS: u64 = 64;
/// Pixels per side of the box one sample averages. Mirrors `BLOCK`.
const BLOCK: u32 = 4;
/// The histogram pass's `@workgroup_size` edge.
const GROUP: u32 = 8;
/// `vec4<f32>`: EV, target, held, unused.
const STATE_BYTES: u64 = 16;

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
                crate::gpu::layout_entries::uniform_entry(0, wgpu::ShaderStages::COMPUTE),
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

/// The readback's state, set from wgpu's thread by the map callback.
const IDLE: u8 = 0;
const MAPPING: u8 = 1;
const MAPPED: u8 = 2;

pub struct ExposureMeter {
    params: wgpu::Buffer,
    histogram: wgpu::Buffer,
    state: wgpu::Buffer,
    staging: wgpu::Buffer,
    mapping: Arc<AtomicU8>,
    stale: bool,
    readout: Option<f32>,
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

    pub(crate) fn step(&mut self, key: HistoryKey, editor: bool, time: f32) -> Step {
        let step = self.clock.step(key, editor, time);
        if step.cut {
            self.forget();
        }
        step
    }

    pub fn cut(&mut self) {
        self.clock.cut();
        self.forget();
    }

    fn forget(&mut self) {
        self.readout = None;
        self.stale = self.mapping.load(Ordering::Acquire) != IDLE;
    }

    pub fn state(&self) -> &wgpu::Buffer {
        &self.state
    }

    pub fn readout(&self) -> Option<f32> {
        self.readout
    }

    pub(crate) fn run(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        commands: &mut Commands,
        (passes, timer): (&ExposurePasses, &GpuTimer),
        (scene, size): (&wgpu::TextureView, [u32; 2]),
        params: &ExposureParams,
    ) {
        self.collect();
        queue.counted_write_buffer(&self.params, 0, bytemuck::bytes_of(params));
        let bind_group = self.bind_group(device, passes, scene);
        let encoder = commands.encoder();
        {
            let mut pass = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Auto Exposure"),
                timestamp_writes: timer.compute_writes(GpuPass::PostFx),
            });
            pass.set_bind_group(crate::gpu::ONLY_GROUP, &bind_group, &[]);
            pass.set_pipeline(&passes.histogram);
            let groups = size.map(|n| n.div_ceil(BLOCK).div_ceil(GROUP).max(1));
            pass.dispatch_workgroups(groups[0], groups[1], 1);
            pass.set_pipeline(&passes.reduce);
            pass.dispatch_workgroups(1, 1, 1);
        }
        if self.mapping.load(Ordering::Acquire) == IDLE {
            encoder.copy_buffer_to_buffer(&self.state, 0, &self.staging, 0, Some(STATE_BYTES));
            // Mapped once the copy is submitted: wgpu refuses a map while it waits.
            self.mapping.store(MAPPING, Ordering::Release);
            let (mapping, staging) = (Arc::clone(&self.mapping), self.staging.clone());
            commands.after_send(move || {
                staging.map_async(wgpu::MapMode::Read, .., move |res| {
                    mapping.store(if res.is_ok() { MAPPED } else { IDLE }, Ordering::Release);
                });
            });
        }
    }

    fn bind_group(
        &self,
        device: &wgpu::Device,
        passes: &ExposurePasses,
        scene: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
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
        })
    }

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

    /// Blocks on `device.poll`: for a capture or a test, never the frame's path.
    pub(crate) fn settle(&mut self, device: &wgpu::Device) {
        if self.mapping.load(Ordering::Acquire) == MAPPING {
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
        }
        self.collect();
    }
}

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod meter_gpu_tests;

#[cfg(test)]
mod stack_gpu_tests;
