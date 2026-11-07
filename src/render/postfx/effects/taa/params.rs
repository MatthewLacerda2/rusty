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
//!   keeps each pixel's own depth beside its colour, and where this pixel lies
//!   behind every depth of last frame's 3×3 at the reprojected place by more than
//!   [`DISOCCLUSION_START`] (relative), the history fades out, gone at
//!   [`DISOCCLUSION_FULL`]. Only behind: a surface last frame held in front of
//!   this one is what left. The 3×3 and the pixel's own (undilated) depth keep a
//!   silhouette or a sub-pixel wire the jitter moves by a pixel from reading as
//!   a reveal. HDRP's *Speed Rejection* (last frame's motion against
//!   this frame's) catches the same reveal but rejects every pixel on the frame a
//!   turn starts or stops, the input a keyboard-turned game sends all the time, so
//!   the depth test stands in for it.
//!
//! A still pixel (under [`STILL_PIXELS`] of motion, nothing revealed) blends
//! exactly as URP's does.

use glam::Vec2;

/// URP's frame influence: the current frame's weight in the blend, for a still
/// pixel.
pub(crate) const FRAME_INFLUENCE: f32 = 0.1;
/// URP's variance clamp scale: the box is the mean ± this many deviations.
pub(crate) const VARIANCE_CLAMP_SCALE: f32 = 0.9;
/// The frame influence of a pixel moving [`MOVING_PIXELS`] or more a frame: UE's
/// TAA range, Post-processing Stack v2's 0.05 → 0.15 ratio (×3) scaled to URP's
/// base weight, rounded down.
pub(crate) const MOVING_FRAME_INFLUENCE: f32 = 0.25;
/// History pixels a frame below which a pixel is still and weighs URP's
/// [`FRAME_INFLUENCE`] exactly: ours, above the rounding a still camera's motion
/// vectors carry (lavapipe moved three pixels of a still frame by one level
/// without it), far below any motion an eye sees.
pub(crate) const STILL_PIXELS: f32 = 0.05;
/// History pixels a frame at which a pixel weighs [`MOVING_FRAME_INFLUENCE`];
/// from [`STILL_PIXELS`] to this the weight rises linearly. Ours: "a few pixels", so a
/// sub-pixel drift (a slow pan, an idle sway) still settles nearly as a still one.
pub(crate) const MOVING_PIXELS: f32 = 2.0;
/// The relative depth gap (`(now - then) / then`, this pixel behind) below which the
/// history is the same surface. Ours, above what a camera's own motion changes
/// a static surface's view depth by in one frame (a 90°/s turn changes it 3 % at
/// 50° off the view axis at 60 fps, 9 % at 20; walking at 5 m/s at 60 fps, 10 %
/// only nearer than 0.8 m).
pub(crate) const DISOCCLUSION_START: f32 = 0.1;
/// The relative depth gap at which the history is dropped: a crate 1 m in
/// front of a wall 5 m away (25 %) reveals the wall with no trace of itself.
pub(crate) const DISOCCLUSION_FULL: f32 = 0.25;

/// The resolve's uniform, `TaaParams` in `postfx/taa.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct TaaParams {
    size: [f32; 4],
    knobs: [f32; 4],
    scene: [f32; 4],
    jitter: [f32; 4],
    rejection: [f32; 4],
}

impl TaaParams {
    /// The history's size `size`, the scene's `scene` (the same when not upscaling)
    /// and the frame's jitter in scene pixels.
    pub(super) fn new(size: (u32, u32), scene: (u32, u32), history: bool, jitter: Vec2) -> Self {
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
                STILL_PIXELS,
            ],
            scene: dims(scene),
            jitter: [jitter.x, jitter.y, 0.0, 0.0],
            rejection: [
                MOVING_FRAME_INFLUENCE,
                MOVING_PIXELS,
                DISOCCLUSION_START,
                DISOCCLUSION_FULL,
            ],
        }
    }
}
