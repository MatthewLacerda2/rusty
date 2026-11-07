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
//!   by URP's frame influence, raised for a moving pixel and toward the frame
//!   alone where the pixel is a reveal (#1846, [`params`]). Its output is what motion blur, the exposure meter,
//!   bloom and the composite read ([`super::super::PostFx::scene_color`]) and next
//!   frame's history;
//! - **the history** ([`TaaHistory`], [`history`]), the view's, in [`super::Targets`]:
//!   two HDR targets read and written in turn, each with the depths its resolve
//!   saw (what tells a reveal, #1846). It is dropped on a resize, a
//!   render-scale change and when TAA stops; it holds no frame after a cut the
//!   motion history names (a scene load, Play or Stop, a camera switch,
//!   `MotionVectors::reset`), so the first frame after one is the current frame
//!   alone, with no ghost of the last.
//!
//! **Upscaling (TAAU, part 3).** With a render scale below 1 the resolve
//! upscales ([`upscale`]): it reconstructs the output-size image from the jittered
//! scene-size frames, at the same place in the chain, and what follows it runs at
//! the output's size, Unity's order (URP's STP, HDRP's FSR2). [`history`] says
//! which path a view takes.
//!
//! **Defaults (URP's `TemporalAA.Settings`).** Frame influence 0.1 for a still pixel
//! (*Base Blend Factor* 0.9; [`params`] for a moving or revealed one), variance clamp scale 0.9, jitter scale 1, mip bias 0 when not
//! upscaling (`log2(scene/output)` when it is, on the material maps only), no
//! sharpening.

mod effect;
pub(crate) mod history;
pub(crate) mod jitter;
pub(crate) mod params;
mod pass;
mod upscale;

pub(crate) use effect::{runs, wanted, EFFECT};
pub(crate) use history::{begin, chain_size, upscales, TaaFrame, TaaHistory};

use params::TaaParams;

use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::PostFx;
use crate::render::timing::{GpuPass, GpuTimer};

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
                texture_entry(5, wgpu::TextureViewDimension::D2),
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
                Fullscreen {
                    label,
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs,
                }
                .colour(device, &pass::targets())
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

    /// The resolve's bind group: `h`'s params, this frame's `current` image, the
    /// history frame `read` (its colour and its depths) and the `velocity` target.
    fn group(
        &self,
        device: &wgpu::Device,
        h: &TaaHistory,
        read: usize,
        current: &wgpu::TextureView,
        velocity: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA Bind Group"),
            layout: &self.layout,
            entries: &[
                entry(0, h.params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(current)),
                entry(2, wgpu::BindingResource::TextureView(&h.frames[read].view)),
                entry(3, wgpu::BindingResource::TextureView(velocity)),
                entry(4, wgpu::BindingResource::Sampler(&self.sampler)),
                entry(5, wgpu::BindingResource::TextureView(&h.depths[read].view)),
            ],
        })
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.resolve, &self.upscale]
    }
}

impl PostFx {
    /// Resolve this frame against the history: at the scene's size, or upscaling
    /// it to the output's (TAAU). `draw_stack` gates the history and the velocity
    /// target on the same `taa::wanted`, so a view holds both or neither.
    pub(crate) fn run_taa(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &TaaPasses,
        timer: &GpuTimer,
        lenses: [[f32; 4]; params::MAX_CAMERAS],
    ) {
        let (Some(h), Some(velocity)) =
            (self.targets.get::<TaaHistory>(), self.motion.target_view())
        else {
            return;
        };
        let (read, write) = (h.latest, 1 - h.latest);
        let params = TaaParams::new((h.size, self.full_size), h.valid, h.pixels, lenses);
        #[cfg(test)]
        let params = params.without(h.rules);
        queue.write_buffer(&h.params, 0, bytemuck::bytes_of(&params));
        let current = self.focused_color();
        let group = passes.group(device, h, read, current, velocity);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("TAA Encoder"),
        });
        let pipeline = if h.upscale {
            &passes.upscale
        } else {
            &passes.resolve
        };
        let timed = (pipeline.get(), timer.writes(GpuPass::Taa));
        let into = [&h.frames[write].view, &h.depths[write].view];
        PostFx::fullscreen_into(&mut encoder, timed, &[&group], into);
        queue.submit(std::iter::once(encoder.finish()));
        let h = self.targets.get_mut::<TaaHistory>().expect("held above");
        (h.latest, h.valid, h.ran) = (write, true, true);
    }

    /// Whether TAA anti-aliased this frame: FXAA then skips, one anti-aliasing.
    pub(crate) fn taa_ran(&self) -> bool {
        self.targets.get::<TaaHistory>().is_some_and(|h| h.ran)
    }

    /// Whether TAA upscaled this frame to the output (TAAU): the chain then wrote
    /// the output itself, and no bilinear upscale followed (`Renderer::render`
    /// decides that up front, through `taa::runs` and `taa::upscales`), for a test.
    #[cfg(test)]
    pub(crate) fn taa_upscaled(&self) -> bool {
        let history = self.targets.get::<TaaHistory>();
        history.is_some_and(|h| h.ran && h.upscale)
    }
}

#[cfg(test)]
mod gpu;

/// The jitter `view` drew its last frame with (zero when it runs no TAA), for a
/// test. A pass never reads it: every camera's jittered matrix is
/// `CameraCtx::view_proj`, the one a pass that builds its own uniform takes.
#[cfg(test)]
pub(crate) fn jitter_of(view: &crate::render::RenderView) -> glam::Vec2 {
    let history = view.post_fx.targets.get::<TaaHistory>();
    history.map_or(glam::Vec2::ZERO, |h| {
        jitter::ndc(h.pixels, view.scene_size())
    })
}

#[cfg(test)]
impl PostFx {
    /// The history's latest frame, for a test to poison.
    pub(crate) fn taa_latest(&self) -> Option<&wgpu::Texture> {
        let h = self.targets.get::<TaaHistory>()?;
        Some(&h.frames[h.latest].texture)
    }
}
