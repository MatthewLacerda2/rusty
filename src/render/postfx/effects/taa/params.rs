//! The resolve's uniform ([`TaaParams`], `TaaParams` in `postfx/taa.wgsl`) and the
//! constants it carries, each beside its source.
//!
//! **What the history keeps and drops (#1846).** URP's TAA, which the resolve
//! follows, drops last frame only where it reprojects off the image or on a cut, and
//! blends the rest at one fixed weight. Two more rules, after HDRP and FSR2:
//! - **A moving pixel trusts its history less**: the frame influence rises from
//!   URP's [`FRAME_INFLUENCE`] to [`MOVING_FRAME_INFLUENCE`] as the pixel's motion
//!   grows to [`MOVING_PIXELS`] (Unity's Post-processing Stack v2 TAA, *Stationary*
//!   to *Motion Blending*; HDRP's TAA keeps the same idea), so a turning view keeps
//!   its detail instead of resampling 90 % of every pixel each frame;
//! - **A revealed surface drops its history** (FSR2's *DepthClip*): the resolve
//!   keeps each pixel's own surface beside its colour, its distance from its
//!   camera and that camera's index in the stack, and where this pixel lies
//!   farther than every surface of last frame's 3×3 at the reprojected place by
//!   more than [`DISOCCLUSION_START`] (relative), the history fades out, gone at
//!   [`DISOCCLUSION_FULL`]. Only farther: a surface last frame held in front of
//!   this one is what left. The distance, not the view depth the velocity target
//!   holds: a camera turn changes a static point's view depth by cos θ_prev /
//!   cos θ_now (a 720°/s flick at 90° FOV, 18 % at the edge), never its distance.
//!   Across cameras the stack order decides, as it does what is in front: a
//!   higher camera's surface gone (a viewmodel bobbing off a wall) is a reveal,
//!   one arriving is not. Three choices keep a still frame's jitter from
//!   reading as a reveal: last frame's 3×3, stepped a scene pixel at a time
//!   (several history pixels under TAAU), so a silhouette the jitter moved by a
//!   pixel finds its surface; this frame's side is the nearest surface in its own
//!   3×3 (FSR2's dilated depth), so a sub-pixel gap the jitter sees the sky
//!   through this frame is no reveal; and the history keeps each pixel's own
//!   surface, undilated, so a sub-pixel wire the jitter misses this frame leaves
//!   sky beside it there. HDRP's *Speed Rejection* (last frame's motion against
//!   this frame's) catches the same reveal but rejects every pixel on the frame a
//!   turn starts or stops, the input a keyboard-turned game sends all the time, so
//!   the depth test stands in for it.
//!
//! A still pixel (under [`STILL_PIXELS`] of motion, nothing revealed) blends
//! exactly as URP's does.

use glam::Vec2;

use crate::components::Projection;
use crate::scene::Camera;

/// URP's frame influence: the current frame's weight in the blend, for a still
/// pixel.
pub(crate) const FRAME_INFLUENCE: f32 = 0.1;
/// URP's variance clamp scale: the box is the mean ± this many deviations.
pub(crate) const VARIANCE_CLAMP_SCALE: f32 = 0.9;
/// The frame influence of a pixel moving [`MOVING_PIXELS`] or more a frame: UE's
/// TAA range, Post-processing Stack v2's 0.05 → 0.15 ratio (×3) scaled to URP's
/// base weight, rounded down.
pub(crate) const MOVING_FRAME_INFLUENCE: f32 = 0.25;
/// How many cameras of a stack the resolve holds a lens for (`MAX_CAMERAS` in
/// `postfx/taa.wgsl`): a camera past the last reads the last one's lens, which
/// misjudges its distances only where the two lenses differ.
pub(crate) const MAX_CAMERAS: usize = 8;
/// History pixels a frame below which a pixel is still and weighs URP's
/// [`FRAME_INFLUENCE`] exactly: ours, above the rounding a still camera's motion
/// vectors carry (lavapipe moved three pixels of a still frame by one level
/// without it), far below any motion an eye sees.
pub(crate) const STILL_PIXELS: f32 = 0.05;
/// History pixels a frame at which a pixel weighs [`MOVING_FRAME_INFLUENCE`];
/// from [`STILL_PIXELS`] to this the weight rises linearly. Ours: "a few pixels", so a
/// sub-pixel drift (a slow pan, an idle sway) still settles nearly as a still one.
pub(crate) const MOVING_PIXELS: f32 = 2.0;
/// The relative distance gap (`(now - then) / then`, this pixel farther) below
/// which the history is the same surface. Ours, above what a camera's own motion
/// changes a static surface's distance by in one frame (a turn: none; walking at
/// 5 m/s at 60 fps, 10 % only nearer than 0.8 m).
pub(crate) const DISOCCLUSION_START: f32 = 0.1;
/// The relative distance gap at which the history is dropped: a crate 1 m in
/// front of a wall 5 m away (25 %) reveals the wall with no trace of itself.
pub(crate) const DISOCCLUSION_FULL: f32 = 0.25;

// The shader divides by both ramps' spans, and a moving pixel weighs more than a
// still one at URP's default: a constant edited out of order fails the build. A
// camera's own Base Blend Factor can lift the still weight past the moving one;
// `TaaParams::new` then raises the moving weight to match.
const _: () = assert!(STILL_PIXELS < MOVING_PIXELS);
const _: () = assert!(DISOCCLUSION_START < DISOCCLUSION_FULL);
const _: () = assert!(FRAME_INFLUENCE < MOVING_FRAME_INFLUENCE && MOVING_FRAME_INFLUENCE <= 1.0);

/// The resolve's uniform, `TaaParams` in `postfx/taa.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct TaaParams {
    size: [f32; 4],
    knobs: [f32; 4],
    scene: [f32; 4],
    jitter: [f32; 4],
    levels: [f32; 4],
    rejection: [f32; 4],
    lenses: [[f32; 4]; MAX_CAMERAS],
}

/// Each of `stack`'s cameras' lens at `aspect`, as the resolve turns a view depth
/// into a distance (`surface_of` in `postfx/taa.wgsl`): the view-space offset per
/// NDC unit at one unit of view depth (perspective, z 1) or at any (orthographic,
/// z 0). A slot past the stack reads as a 90° square lens; nothing indexes it.
pub(super) fn lenses(stack: &[Camera], aspect: f32) -> [[f32; 4]; MAX_CAMERAS] {
    let mut out = [[1.0, 1.0, 1.0, 0.0]; MAX_CAMERAS];
    for (slot, cam) in out.iter_mut().zip(stack) {
        let p = cam.projection_matrix(aspect);
        let perspective = matches!(cam.projection, Projection::Perspective);
        *slot = [
            1.0 / p.x_axis.x,
            1.0 / p.y_axis.y,
            f32::from(u8::from(perspective)),
            0.0,
        ];
    }
    out
}

/// The camera's TAA knobs the resolve reads (#1847): the still pixel's frame
/// influence (`1 - base_blend`), the variance clamp scale and the quality level's
/// steps ([`levels`]). URP's defaults reduce to the constants [`FRAME_INFLUENCE`],
/// [`VARIANCE_CLAMP_SCALE`] and `High`, so a camera left at its defaults resolves
/// bit for bit as before (#1847).
#[derive(Clone, Copy, Debug)]
pub(crate) struct TaaTune {
    pub frame_influence: f32,
    pub variance_clamp_scale: f32,
    pub levels: [f32; 4],
}

impl TaaTune {
    /// From a camera's TAA fields: frame influence is URP's `1 - base_blend`.
    pub(crate) fn from_camera(cam: &crate::components::CameraComponent) -> Self {
        Self {
            frame_influence: (1.0 - cam.taa_base_blend_factor).clamp(0.0, 1.0),
            variance_clamp_scale: cam.taa_variance_clamp_scale,
            levels: levels(cam.taa_quality),
        }
    }
}

impl Default for TaaTune {
    /// A default camera's tune: URP's defaults, so the resolve matches the constants.
    fn default() -> Self {
        Self {
            frame_influence: FRAME_INFLUENCE,
            variance_clamp_scale: VARIANCE_CLAMP_SCALE,
            levels: levels(crate::components::TaaQuality::default()),
        }
    }
}

/// A quality level's steps as the shader reads them (`taa.levels`): URP's
/// `DoTemporalAA(clamp, motion, history, central)` arguments per level
/// (`TemporalAA.shader`'s passes):
/// - **clamp** 0 the 5-tap cross's min/max box, 2 the 3×3's narrowed by the variance,
///   3 that box with the history clipped toward its centre (`ClipToAABBCenter`)
///   rather than clamped;
/// - **motion** 0 the pixel's own velocity, 1 the frontmost of its 5-tap cross,
///   2 of its 3×3;
/// - **history** 0 and 1 one bilinear read (rusty drops an off-image history at
///   every level, where URP's 0 keeps it), 2 the 5-tap bicubic;
/// - **central** 1 re-filters the current pixel toward its jittered average.
pub(crate) fn levels(quality: crate::components::TaaQuality) -> [f32; 4] {
    use crate::components::TaaQuality::*;
    match quality {
        VeryLow => [0.0, 0.0, 0.0, 0.0],
        Low => [0.0, 1.0, 1.0, 0.0],
        Medium => [2.0, 2.0, 1.0, 0.0],
        High => [2.0, 2.0, 2.0, 0.0],
        VeryHigh => [3.0, 2.0, 2.0, 1.0],
    }
}

impl TaaParams {
    /// The history's size `size`, the scene's `scene` (the same when not upscaling),
    /// whether the history holds a frame, the frame's jitter in scene pixels,
    /// whether a translucent wrote the reactive mask (#1845), the camera's [`TaaTune`]
    /// (#1847) and the stack's [`lenses`].
    pub(super) fn new(
        (size, scene): ((u32, u32), (u32, u32)),
        (history, jitter, reactive): (bool, Vec2, bool),
        tune: TaaTune,
        lenses: [[f32; 4]; MAX_CAMERAS],
    ) -> Self {
        let dims = |(w, h): (u32, u32)| {
            let (w, h) = (w.max(1) as f32, h.max(1) as f32);
            [w, h, 1.0 / w, 1.0 / h]
        };
        Self {
            size: dims(size),
            knobs: [
                tune.frame_influence,
                tune.variance_clamp_scale,
                f32::from(u8::from(history)),
                STILL_PIXELS,
            ],
            scene: dims(scene),
            jitter: [jitter.x, jitter.y, f32::from(u8::from(reactive)), 0.0],
            levels: tune.levels,
            rejection: [
                // A moving pixel never trusts its history more than a still one,
                // whatever the camera's Base Blend Factor (#1847).
                MOVING_FRAME_INFLUENCE.max(tune.frame_influence),
                MOVING_PIXELS,
                DISOCCLUSION_START,
                DISOCCLUSION_FULL,
            ],
            lenses,
        }
    }

    /// The same frame with #1846's two rules off, for a test: a moving pixel weighs
    /// a still one's, `moving` false, and no gap reads as a reveal, `reveal` false.
    #[cfg(test)]
    pub(super) fn without(mut self, rules: Rules) -> Self {
        if !rules.moving {
            self.rejection[0] = self.knobs[0];
        }
        if !rules.reveal {
            // Out of reach of any gap, yet a finite span to divide by.
            self.rejection[2..4].copy_from_slice(&[1e30, 2e30]);
        }
        self
    }
}

/// Which of #1846's rules a view's resolve keeps, for a test that compares a frame
/// with one ([`TaaParams::without`]).
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rules {
    pub moving: bool,
    pub reveal: bool,
}

#[cfg(test)]
impl Default for Rules {
    fn default() -> Self {
        Self {
            moving: true,
            reveal: true,
        }
    }
}

#[cfg(test)]
#[path = "levels_tests.rs"]
mod levels_tests;
#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
