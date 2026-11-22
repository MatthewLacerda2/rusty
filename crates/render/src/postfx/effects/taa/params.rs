//! Not HDRP's Speed Rejection, which drops all history whenever a keyboard turn starts or
//! stops; a depth test finds revealed surfaces instead.

use glam::Vec2;

use rusty_components::Projection;
use rusty_scene::Camera;

pub(crate) const FRAME_INFLUENCE: f32 = 0.1;
pub(crate) const MOVING_FRAME_INFLUENCE: f32 = 0.25;
pub(crate) const MAX_CAMERAS: usize = 8;
/// Above the rounding a still camera's motion vectors carry (lavapipe moved still pixels
/// without it).
pub(crate) const STILL_PIXELS: f32 = 0.05;
pub(crate) const MOVING_PIXELS: f32 = 2.0;
/// Gaps in eye distance, not view depth: a turn changes view depth by a cosine ratio (about
/// 18% at the edge), never distance.
pub(crate) const DISOCCLUSION_START: f32 = 0.1;
pub(crate) const DISOCCLUSION_FULL: f32 = 0.25;
pub(crate) const FLICKER_DECAY: f32 = 0.9;
pub(crate) const FLICKER_HOLD: f32 = 4.0;

const _: () = assert!(STILL_PIXELS < MOVING_PIXELS);
const _: () = assert!(DISOCCLUSION_START < DISOCCLUSION_FULL);
const _: () = assert!(FRAME_INFLUENCE < MOVING_FRAME_INFLUENCE && MOVING_FRAME_INFLUENCE <= 1.0);
const _: () = assert!(0.0 <= FLICKER_DECAY && FLICKER_DECAY < 1.0 && FLICKER_HOLD >= 0.0);

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct TaaParams {
    size: [f32; 4],
    knobs: [f32; 4],
    scene: [f32; 4],
    jitter: [f32; 4],
    levels: [f32; 4],
    rejection: [f32; 4],
    flicker: [f32; 4],
    lenses: [[f32; 4]; MAX_CAMERAS],
}

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

#[derive(Clone, Copy, Debug)]
pub(crate) struct TaaTune {
    pub frame_influence: f32,
    pub variance_clamp_scale: f32,
    pub levels: [f32; 4],
    pub anti_flicker: f32,
}

impl TaaTune {
    pub(crate) fn from_camera(cam: &rusty_components::CameraComponent) -> Self {
        Self {
            frame_influence: (1.0 - cam.taa_base_blend_factor).clamp(0.0, 1.0),
            variance_clamp_scale: cam.taa_variance_clamp_scale,
            levels: levels(cam.taa_quality),
            anti_flicker: cam.taa_anti_flicker.clamp(0.0, 1.0),
        }
    }
}

impl Default for TaaTune {
    fn default() -> Self {
        Self::from_camera(&rusty_components::CameraComponent::default())
    }
}

/// `taa.wgsl` compares these as bare numbers: clamp 0 cross, 2 variance, 3 clip; motion 0 own,
/// 1 cross, 2 3×3; history 2 bicubic; central 1 refilter.
pub(crate) fn levels(quality: rusty_components::TaaQuality) -> [f32; 4] {
    use rusty_components::TaaQuality::*;
    match quality {
        VeryLow => [0.0, 0.0, 0.0, 0.0],
        Low => [0.0, 1.0, 1.0, 0.0],
        Medium => [2.0, 2.0, 1.0, 0.0],
        High => [2.0, 2.0, 2.0, 0.0],
        VeryHigh => [3.0, 2.0, 2.0, 1.0],
    }
}

impl TaaParams {
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
                MOVING_FRAME_INFLUENCE.max(tune.frame_influence),
                MOVING_PIXELS,
                DISOCCLUSION_START,
                DISOCCLUSION_FULL,
            ],
            flicker: [FLICKER_DECAY, FLICKER_HOLD * tune.anti_flicker, 0.0, 0.0],
            lenses,
        }
    }

    #[cfg(test)]
    pub(super) fn without(mut self, rules: Rules) -> Self {
        if !rules.moving {
            self.rejection[0] = self.knobs[0];
        }
        if !rules.reveal {
            self.rejection[2..4].copy_from_slice(&[1e30, 2e30]);
        }
        self
    }
}

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
