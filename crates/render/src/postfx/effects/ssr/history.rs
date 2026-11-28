//! HDRP's *PBR Accumulation*: the frame before's reflection and each pixel's distance,
//! which `fs_accumulate` reads and rewrites, ping-ponged.

use glam::Vec3;

use super::pass::{SsrParams, SsrPasses, INPUTS, TRACED};
use super::record::{begin, opaque_target, pipeline_layout, Stamps};
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::texture_entry;
use crate::gpu::ops::CountedDevice as _;
use crate::gpu::pipelines::LazyPipeline;
use crate::postfx::{PostFx, HDR_FORMAT};

const HISTORY: u32 = 2;

/// f16 holds a 2 % gap to a few hundred metres.
pub(super) const DISTANCE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

pub(crate) struct Accumulate {
    layout: wgpu::BindGroupLayout,
    pub(super) pipeline: LazyPipeline,
}

impl Accumulate {
    pub(super) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        inputs: [&wgpu::BindGroupLayout; 2],
    ) -> Self {
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR History Layout"),
            entries: &[texture_entry(0, d2), texture_entry(1, d2)],
        });
        let groups = pipeline_layout(device, &[inputs[0], inputs[1], &layout]);
        let shader = shader.clone();
        let pipeline = LazyPipeline::new(device, move |device| {
            let formats = [HDR_FORMAT, HDR_FORMAT, DISTANCE_FORMAT];
            Fullscreen {
                label: "SSR Accumulate Pipeline",
                shader: &shader,
                layout: &groups,
                vs: "vs_fullscreen",
                fs: "fs_accumulate",
            }
            .colour(device, &formats.map(|f| Some(opaque_target(f))))
        });
        Self { layout, pipeline }
    }

    /// Writes every pixel: one that takes no SSR keeps its colour and clears its history.
    pub(super) fn record<'e>(
        &self,
        encoder: &'e mut wgpu::CommandEncoder,
        (inputs, traced): (&wgpu::BindGroup, &wgpu::BindGroup),
        (scene, h): (&'e wgpu::TextureView, Turn<'e>),
        timed: Stamps<'e>,
    ) {
        let clear = wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT);
        let [reflection, distance] = h.write;
        let targets = [
            (scene, wgpu::LoadOp::Load),
            (reflection, clear),
            (distance, clear),
        ];
        let mut pass = begin(encoder, "SSR Accumulate", &targets, timed);
        pass.set_pipeline(self.pipeline.get());
        pass.set_bind_group(INPUTS, inputs, &[]);
        pass.set_bind_group(TRACED, traced, &[]);
        pass.set_bind_group(HISTORY, h.read, &[]);
        pass.draw(0..3, 0..1);
    }
}

impl SsrParams {
    pub(super) fn accumulates(&self) -> bool {
        self.surface[2] > 0.0
    }

    pub(super) fn eye(&self) -> Vec3 {
        Vec3::from_slice(&self.camera_pos[..3])
    }

    /// Reads the frame before's reflection, written from `eye`.
    pub(super) fn holding(&mut self, eye: Vec3) {
        self.history = eye.extend(1.0).to_array();
    }
}

impl PostFx {
    /// Returns whether this frame is a cut.
    pub(super) fn ssr_history_turn(
        &mut self,
        device: &wgpu::Device,
        passes: &SsrPasses,
        params: &mut SsrParams,
    ) -> bool {
        self.fit_ssr_history(device, passes, params.accumulates());
        let cut = self.motion.history.cut();
        let history = self.targets.get::<SsrHistory>();
        if let Some(eye) = history.and_then(|h| h.turn(cut).eye) {
            params.holding(eye);
        }
        cut
    }

    fn fit_ssr_history(&mut self, device: &wgpu::Device, passes: &SsrPasses, on: bool) {
        if !on {
            self.targets.remove::<SsrHistory>();
            return;
        }
        let (size, layout) = (self.full_size, &passes.accumulate.layout);
        let h = (self.targets).get_or(|| SsrHistory::new(device, layout, size));
        if h.size != size {
            *h = SsrHistory::new(device, layout, size);
        }
    }
}

struct Frame {
    reflection: wgpu::TextureView,
    distance: wgpu::TextureView,
    read: wgpu::BindGroup,
}

pub(crate) struct SsrHistory {
    pub(super) size: (u32, u32),
    frames: [Frame; 2],
    latest: usize,
    /// The camera `latest` was written from.
    eye: Option<Vec3>,
}

/// This frame's read group, the pair it writes, and the camera position the read was written
/// from (`None`: no history to read).
pub(super) struct Turn<'a> {
    pub read: &'a wgpu::BindGroup,
    pub write: [&'a wgpu::TextureView; 2],
    pub eye: Option<Vec3>,
}

impl SsrHistory {
    pub(super) fn new(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        size: (u32, u32),
    ) -> Self {
        let frame = |i: usize| {
            let make = |label: &str, format| {
                use wgpu::TextureUsages as U;
                let t = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
                    view_formats: &[],
                });
                t.create_view(&Default::default())
            };
            let reflection = make(&format!("SSR History {i}"), HDR_FORMAT);
            let distance = make(&format!("SSR History Distance {i}"), DISTANCE_FORMAT);
            let read = device.counted_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SSR History Bind Group"),
                layout,
                entries: &[
                    entry(0, wgpu::BindingResource::TextureView(&reflection)),
                    entry(1, wgpu::BindingResource::TextureView(&distance)),
                ],
            });
            Frame {
                reflection,
                distance,
                read,
            }
        };
        Self {
            size,
            frames: [frame(0), frame(1)],
            latest: 0,
            eye: None,
        }
    }

    /// The frame's turn; `cut` (a camera cut, a scene swap) forgets what was kept.
    pub(super) fn turn(&self, cut: bool) -> Turn<'_> {
        let (read, write) = (&self.frames[self.latest], &self.frames[1 - self.latest]);
        Turn {
            read: &read.read,
            write: [&write.reflection, &write.distance],
            eye: self.eye.filter(|_| !cut),
        }
    }

    pub(super) fn wrote(&mut self, eye: Vec3) {
        self.latest = 1 - self.latest;
        self.eye = Some(eye);
    }
}
