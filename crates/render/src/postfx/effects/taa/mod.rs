//! crates/render/src/postfx/effects/taa/ — temporal anti-aliasing (#1702): a **camera**
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
//!   two HDR targets read and written in turn, each with the surfaces its
//!   resolve saw, distance and camera (what tells a reveal, #1846), and, with the
//!   camera's anti-flicker on, each pixel's flicker (what holds a still pixel
//!   blinking under the jitter, #2228, [`params`]). It is dropped on a resize, a
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
//! sharpening. The camera tunes each (#1847, [`params::TaaTune`]), and its
//! contrast-adaptive sharpen is TAA's second pass, an LDR one after the
//! composite ([`sharpen`]).

mod effect;
pub(crate) mod history;
pub(crate) mod jitter;
pub(crate) mod params;
mod pass;
pub(crate) mod reactive;
pub(crate) mod sharpen;
mod upscale;

use crate::gpu::ops::CountedQueue as _;
pub(crate) use effect::{runs, wanted, EFFECT};
pub(crate) use history::{begin, chain_size, upscales, TaaFrame, TaaHistory};
pub(crate) use sharpen::Sharpen;

use params::TaaParams;
pub(crate) use pass::TaaPasses;

use crate::postfx::PostFx;
use crate::timing::{GpuPass, GpuTimer};

impl PostFx {
    /// Resolve this frame against the history: at the scene's size, or upscaling
    /// it to the output's (TAAU). `draw_stack` gates the history and the velocity
    /// target on the same `taa::wanted`, so a view holds both or neither. With the
    /// camera's anti-flicker on (#2228) the history holds its flicker frames and the
    /// held pipeline runs; at 0 they are dropped and URP's resolve runs.
    pub(crate) fn run_taa(
        &mut self,
        (device, queue): (&wgpu::Device, &wgpu::Queue),
        encoder: &mut wgpu::CommandEncoder,
        passes: &TaaPasses,
        timer: &GpuTimer,
        (tune, lenses): (params::TaaTune, [[f32; 4]; params::MAX_CAMERAS]),
    ) {
        if let Some(h) = self.targets.get_mut::<TaaHistory>() {
            h.hold_flickers(device, tune.anti_flicker > 0.0);
        }
        let (Some(h), Some(velocity)) =
            (self.targets.get::<TaaHistory>(), self.motion.target_view())
        else {
            return;
        };
        let (read, write) = (h.latest, 1 - h.latest);
        let drawn = self.take_reactive_drawn();
        let frame = (h.valid, h.pixels, drawn);
        let sizes = (h.size, self.full_size); // chain-size: scene the frame TAA reads
        let params = TaaParams::new(sizes, frame, tune, lenses);
        #[cfg(test)]
        let params = params.without(h.rules);
        queue.counted_write_buffer(&h.params, 0, bytemuck::bytes_of(&params));
        let current = self.focused_color();
        let group = passes.group(device, h, read, current, velocity);
        let pipeline = match (h.upscale, h.flickers.is_some()) {
            (false, false) => &passes.resolve,
            (true, false) => &passes.upscale,
            (false, true) => &passes.resolve_held,
            (true, true) => &passes.upscale_held,
        };
        let timed = (pipeline.get(), timer.writes(GpuPass::Taa));
        let (colour, surface) = (&h.frames[write].view, &h.surfaces[write].view);
        match &h.flickers {
            None => PostFx::fullscreen_into(encoder, timed, &[&group], [colour, surface]),
            Some(flickers) => {
                let into = [colour, surface, &flickers[write].view];
                PostFx::fullscreen_into(encoder, timed, &[&group], into);
            }
        }
        if drawn {
            reactive::clear(encoder, &h.reactive, timer.writes(GpuPass::Taa));
        }
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
pub(in crate::postfx::effects) mod gpu;

/// The jitter `view` drew its last frame with (zero when it runs no TAA), for a
/// test. A pass never reads it: every camera's jittered matrix is
/// `CameraCtx::view_proj`, the one a pass that builds its own uniform takes.
#[cfg(test)]
pub(crate) fn jitter_of(view: &crate::RenderView) -> glam::Vec2 {
    let history = view.post_fx.targets.get::<TaaHistory>();
    history.map_or(glam::Vec2::ZERO, |h| {
        jitter::ndc(h.pixels, view.scene_size()) // chain-size: scene the jitter's
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
