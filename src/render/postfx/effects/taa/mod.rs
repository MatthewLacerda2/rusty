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
//! - **the history** ([`TaaHistory`]), the view's, in [`super::Targets`]: two HDR
//!   targets at the view's scene size, read and written in turn. It is dropped on a
//!   resize and when TAA stops; it holds no frame after a cut the motion history
//!   names (a scene load, Play or Stop, a camera switch,
//!   `MotionVectors::reset`), so the first frame after one is the current frame
//!   alone, with no ghost of the last.
//!
//! **Size.** The jitter and the resolve are in scene-size pixels, so a render scale
//! below 1 resolves at the scaled size and is upscaled after, as every other post
//! target is. Upscaling inside the resolve (TAAU) is #1702's part 3.
//!
//! **Defaults (URP's `TemporalAA.Settings`).** Frame influence 0.1 (*Base Blend
//! Factor* 0.9), variance clamp scale 0.9, jitter scale 1, mip bias 0 (no texture
//! LOD bias: `lightmap_level` and the specular AA's derivatives see the frame as
//! drawn), no sharpening.

mod effect;
pub(crate) mod jitter;

pub(crate) use effect::{wanted, EFFECT};

use glam::Vec2;

use super::Targets;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};
use crate::render::{PixelSize, RenderView};

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
}

impl TaaParams {
    fn new((w, h): (u32, u32), history: bool) -> Self {
        let (w, h) = (w as f32, h as f32);
        Self {
            size: [w, h, 1.0 / w, 1.0 / h],
            knobs: [
                FRAME_INFLUENCE,
                VARIANCE_CLAMP_SCALE,
                f32::from(u8::from(history)),
                0.0,
            ],
        }
    }
}

/// The resolve's pipeline, shared by every view.
pub(crate) struct TaaPasses {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    resolve: LazyPipeline,
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
        let resolve = LazyPipeline::new(device, move |device| {
            let target = wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            };
            Fullscreen {
                label: "TAA Resolve",
                shader: &shader,
                layout: &pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_resolve",
            }
            .colour(device, &[Some(target)])
        });
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
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 1] {
        [&self.resolve]
    }
}

/// One view's TAA state while it runs: the two history targets, the frame's
/// jitter and whether the history holds a resolved frame.
pub(crate) struct TaaHistory {
    size: (u32, u32),
    frames: [PfxTarget; 2],
    params: wgpu::Buffer,
    /// Which of `frames` holds the latest resolve.
    latest: usize,
    /// Whether `frames[latest]` is a frame of this view (not a fresh target).
    valid: bool,
    /// The jitter sequence's frame counter.
    frame: u32,
    /// This frame's jitter, in NDC.
    jitter: Vec2,
    /// Whether the resolve ran this frame: its output is then the scene.
    ran: bool,
}

impl TaaHistory {
    fn new(device: &wgpu::Device, (w, h): (u32, u32)) -> Self {
        let frame = |label| PostFx::target(device, w, h, label, HDR_FORMAT);
        Self {
            size: (w, h),
            frames: [frame("TAA History A"), frame("TAA History B")],
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("TAA Params"),
                size: std::mem::size_of::<TaaParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            latest: 0,
            valid: false,
            frame: 0,
            jitter: Vec2::ZERO,
            ran: false,
        }
    }

    /// This frame's resolve, when it ran: what the rest of the chain reads.
    pub(crate) fn output(&self) -> Option<&PfxTarget> {
        self.ran.then(|| &self.frames[self.latest])
    }
}

/// Start a stack's frame: with `on`, fit the view's history to `size` and return
/// this frame's jitter in NDC; without, drop the history and return none. A view
/// that runs no TAA draws with the unjittered projection, bit for bit.
pub(crate) fn begin(
    targets: &mut Targets,
    device: &wgpu::Device,
    size: PixelSize,
    on: bool,
) -> Vec2 {
    if !on {
        targets.remove::<TaaHistory>();
        return Vec2::ZERO;
    }
    let size_px = (size.width.max(1), size.height.max(1));
    let h = targets.get_or(|| TaaHistory::new(device, size_px));
    if h.size != size_px {
        *h = TaaHistory::new(device, size_px);
    }
    h.frame = h.frame.wrapping_add(1);
    h.ran = false;
    h.jitter = jitter::ndc(jitter::pixels(h.frame), size);
    h.jitter
}

/// The jitter `view` draws this frame with (zero when it runs no TAA): what a pass
/// building its own view-projection (particles, ribbons, world UI) applies through
/// [`jitter::jittered`], so it lines up with the depth the solids wrote.
pub(crate) fn jitter_of(view: &RenderView) -> Vec2 {
    let history = view.post_fx.targets.get::<TaaHistory>();
    history.map_or(Vec2::ZERO, |h| h.jitter)
}

impl PostFx {
    /// Resolve this frame against the history, or drop the history when the view
    /// wrote no velocity. `cut`: the motion history starts over this frame, so the
    /// history holds nothing of this view.
    pub(crate) fn run_taa(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        passes: &TaaPasses,
        cut: bool,
        timer: &GpuTimer,
    ) {
        if self.motion.target_view().is_none() {
            self.targets.remove::<TaaHistory>();
        }
        let (Some(h), Some(velocity)) =
            (self.targets.get::<TaaHistory>(), self.motion.target_view())
        else {
            return;
        };
        let (read, write) = (h.latest, 1 - h.latest);
        let params = TaaParams::new(h.size, h.valid && !cut);
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

    /// Whether the TAA resolve ran this frame (FXAA then skips: one anti-aliasing).
    pub(crate) fn taa_ran(&self) -> bool {
        self.targets.get::<TaaHistory>().is_some_and(|h| h.ran)
    }
}

#[cfg(test)]
mod ghost_gpu_tests;
#[cfg(test)]
mod gpu_fixture;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod shimmer_gpu_tests;

#[cfg(test)]
impl PostFx {
    /// The history's latest frame, for a test to poison.
    pub(crate) fn taa_latest(&self) -> Option<&wgpu::Texture> {
        let h = self.targets.get::<TaaHistory>()?;
        Some(&h.frames[h.latest].texture)
    }
}
