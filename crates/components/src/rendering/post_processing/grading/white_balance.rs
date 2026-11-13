//! White Balance (#1440), Unity URP's override of that name: baked into the view's
//! grading LUT, first.

use serde::{Deserialize, Serialize};

/// White balance (Unity's *White Balance* override): shifts the image's white point.
/// Both are `-100..=100`, neutral `0`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WhiteBalance {
    /// Positive warms (towards yellow), negative cools (towards blue).
    pub temperature: f32,
    /// Positive tints towards magenta, negative towards green.
    pub tint: f32,
}

impl WhiteBalance {
    /// How far either knob reaches from neutral.
    pub const RANGE: f32 = 100.0;

    /// Both knobs at neutral: the bake skips the white-point transform.
    pub fn is_neutral(&self) -> bool {
        self.temperature == 0.0 && self.tint == 0.0
    }
}
