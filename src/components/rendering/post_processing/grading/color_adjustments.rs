//! Color Adjustments, Unity URP's override of that name: contrast, saturation and
//! the hue shift (#1440). The renderer bakes them into the view's grading LUT, all on
//! the scene-linear colour before the tonemap, as URP does (#1479).

use serde::{Deserialize, Serialize};

/// Contrast, saturation and the hue shift; each neutral at its default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorAdjustments {
    /// Spreads the colour away from mid grey (`> 1`) or towards it, in log space;
    /// `0..=2`.
    pub contrast: f32,
    /// `0` is greyscale, `1` neutral, up to `2` more saturated.
    pub saturation: f32,
    /// Rotates every hue by this many degrees, `-180..=180`.
    pub hue_shift: f32,
}

impl ColorAdjustments {
    /// The top of contrast's and saturation's range: Unity's `+100` (#1479).
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
