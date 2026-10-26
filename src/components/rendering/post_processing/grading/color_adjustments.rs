//! Color Adjustments, Unity URP's override of that name: contrast, saturation and
//! the hue shift (#1440). The renderer bakes them into the view's grading LUT: the
//! hue shift on the scene-linear colour before the tonemap, contrast and saturation
//! on the tonemapped one.

use serde::{Deserialize, Serialize};

/// Contrast, saturation and the hue shift; each neutral at its default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorAdjustments {
    /// Spreads the tonemapped colour away from mid grey (`> 1`) or towards it.
    pub contrast: f32,
    /// `0` is greyscale, `1` neutral, above `1` more saturated.
    pub saturation: f32,
    /// Rotates every hue by this many degrees, `-180..=180`.
    pub hue_shift: f32,
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
