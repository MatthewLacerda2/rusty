//! Unity URP's Color Adjustments, baked into the grading LUT on scene-linear colour before the
//! tonemap.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorAdjustments {
    /// In log space, around mid grey; `0..=2`.
    pub contrast: f32,
    /// `0` is greyscale, `1` neutral, up to `2` more saturated.
    pub saturation: f32,
    /// Rotates every hue by this many degrees, `-180..=180`.
    pub hue_shift: f32,
}

impl ColorAdjustments {
    /// Unity's `+100`.
    pub const MAX: f32 = 2.0;
}

impl Default for ColorAdjustments {
    fn default() -> Self {
        Self {
            contrast: 1.0,
            saturation: 1.0,
            hue_shift: 0.0,
        }
    }
}
