//! src/render/postfx/effects/taa/ — temporal anti-aliasing (#1702): a **camera**
//! effect, URP's TAA at its default quality, in `postfx/taa.wgsl`.
//!
//! Three parts:
//! - **the jitter** ([`jitter`]), chosen when the stack starts ([`begin`]): every
//!   camera of the stack draws with the projection moved a sub-pixel, the next point
//!   of URP's Halton (2, 3) sequence;
//! - **the resolve**, a scene pass after depth of field and before motion blur
//!   (URP's `PostProcessPass` order: depth of field, TAA, motion blur, then bloom
//!   and the uber pass): the current frame's neighbourhood box, the history
//!   reprojected through the view's motion vectors and clamped into it, blended in
//!   by URP's frame influence. Its output is what motion blur, the exposure meter,
//!   bloom and the composite read ([`super::super::PostFx::scene_color`]) and next
//!   frame's history;
//! - **the history** ([`TaaHistory`], [`history`]), the view's, in [`super::Targets`]:
//!   two HDR targets read and written in turn. It is dropped on a resize, a
//!   render-scale change and when TAA stops; it holds no frame after a cut the
//!   motion history names (a scene load, Play or Stop, a camera switch,
//!   `MotionVectors::reset`), so the first frame after one is the current frame
//!   alone, with no ghost of the last.
//!
//! **Upscaling (TAAU, part 3).** With a render scale below 1 the view upscales
//! inside TAA instead of bilinearly ([`upscale`]): the post chain runs at the
//! scene's size as it always did, and the resolve then reconstructs the
//! post-processed image at the output's size from the jittered samples of many
//! frames, in place of the bilinear upscale. [`history`] says which path a view
//! takes.
//!
//! **Defaults (URP's `TemporalAA.Settings`).** Frame influence 0.1 (*Base Blend
//! Factor* 0.9), variance clamp scale 0.9, jitter scale 1, mip bias 0 when not
//! upscaling (`log2(scene/output)` when it is, on the material maps only), no
//! sharpening.

mod effect;
pub(crate) mod history;
pub(crate) mod jitter;
mod upscale;

pub(crate) use effect::{wanted, EFFECT};
pub(crate) use history::{begin, TaaFrame, TaaHistory};

use glam::Vec2;

use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// URP's frame influence: the current frame's weight in the blend.
pub(crate) const FRAME_INFLUENCE: f32 = 0.1;
/// URP's variance clamp scale: the box is the mean ± this many deviations.
pub(crate) const VARIANCE_CLAMP_SCALE: f32 = 0.9;

/// The resolve's uniform, `TaaParams` in `postfx/taa.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TaaParams {
    size: [f32; 4],
    knobs: [f32; 4],
    scene: [f32; 4],
    jitter: [f32; 4],
}

impl TaaParams {
    /// The history's size `size`, the scene's `scene` (the same when not upscaling)
    /// and the frame's jitter in scene pixels.
    fn new(size: (u32, u32), scene: (u32, u32), history: bool, jitter: Vec2) -> Self {
        let dims = |(w, h): (u32, u32)| {
            let (w, h) = (w.max(1) as f32, h.max(1) as f32);
            [w, h, 1.0 / w, 1.0 / h]
        };
        Self {
            size: dims(size),
            knobs: [
                FRAME_INFLUENCE,
                VARIANCE_CLAMP_SCALE,
                f32::from(u8::from(history)),
                0.0,
            ],
            scene: dims(scene),
            jitter: [jitter.x, jitter.y, 0.0, 0.0],
        }
    }
}

/// The resolve's pipelines, shared by every view: the scene-size resolve and the
/// upscaling one (TAAU), both into an HDR history target.
pub(crate) struct TaaPasses {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    resolve: LazyPipeline,
    upscale: LazyPipeline,
}

impl TaaPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/taa.wgsl", "TAA Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TAA Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
                texture_entry(2, wgpu::TextureViewDimension::D2),
                texture_entry(3, wgpu::TextureViewDimension::D2),
                sampler_entry(4),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TAA Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |fs: &'static str, label: &'static str| {
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            LazyPipeline::new(device, move |device| {
                let target = wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                };
                Fullscreen {
                    label,
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs,
                }
                .colour(device, &[Some(target)])
            })
        };
        let resolve = pipeline("fs_resolve", "TAA Resolve");
        let upscale = pipeline("fs_upscale", "TAA Upscale");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TAA History Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            layout,
            sampler,
            resolve,
            upscale,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.resolve, &self.upscale]
    }
}

impl PostFx {
    /// Resolve this frame against the history. `draw_stack` gates the history
    /// and the velocity target on the same `taa::wanted`, so a view holds both or
    /// neither.
    pub(crate) fn run_taa(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &TaaPasses,
        timer: &GpuTimer,
    ) {
        let (Some(h), Some(velocity)) =
            (self.targets.get::<TaaHistory>(), self.motion.target_view())
        else {
            return;
        };
        // An upscaling view resolves after the post chain instead (`taa_upscale`).
        if h.upscale {
            return;
        }
        let (read, write) = (h.latest, 1 - h.latest);
        let params = TaaParams::new(h.size, h.size, h.valid, h.pixels);
        queue.write_buffer(&h.params, 0, bytemuck::bytes_of(&params));
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA Bind Group"),
            layout: &passes.layout,
            entries: &[
                entry(0, h.params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(self.focused_color())),
                entry(2, wgpu::BindingResource::TextureView(&h.frames[read].view)),
                entry(3, wgpu::BindingResource::TextureView(velocity)),
                entry(4, wgpu::BindingResource::Sampler(&passes.sampler)),
            ],
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("TAA Encoder"),
        });
        let timed = (passes.resolve.get(), timer.writes(GpuPass::Taa));
        Self::fullscreen(&mut encoder, timed, &[&group], &h.frames[write].view);
        queue.submit(std::iter::once(encoder.finish()));
        let h = self.targets.get_mut::<TaaHistory>().expect("held above");
        (h.latest, h.valid, h.ran) = (write, true, true);
    }

    /// Whether TAA anti-aliases this frame: its scene-size resolve ran, or the view
    /// upscales through it after the chain. FXAA then skips: one anti-aliasing.
    pub(crate) fn taa_ran(&self) -> bool {
        self.targets
            .get::<TaaHistory>()
            .is_some_and(|h| h.ran || h.upscale)
    }
}

#[cfg(test)]
mod cut_gpu_tests;
#[cfg(test)]
mod ghost_gpu_tests;
#[cfg(test)]
mod gpu_fixture;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod shimmer_gpu_tests;
#[cfg(test)]
mod viewmodel_gpu_tests;

/// The jitter `view` drew its last frame with (zero when it runs no TAA), for a
/// test. A pass never reads it: every camera's jittered matrix is
/// `CameraCtx::view_proj`, the one a pass that builds its own uniform takes.
#[cfg(test)]
pub(crate) fn jitter_of(view: &crate::render::RenderView) -> Vec2 {
    let history = view.post_fx.targets.get::<TaaHistory>();
    history.map_or(Vec2::ZERO, |h| jitter::ndc(h.pixels, view.scene_size()))
}

#[cfg(test)]
impl PostFx {
    /// The history's latest frame, for a test to poison.
    pub(crate) fn taa_latest(&self) -> Option<&wgpu::Texture> {
        let h = self.targets.get::<TaaHistory>()?;
        Some(&h.frames[h.latest].texture)
    }
}
