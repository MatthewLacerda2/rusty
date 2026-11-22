use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vignette {
    pub active: bool,
    /// `0..=1`; `0` draws nothing.
    pub intensity: f32,
    pub smoothness: f32,
    /// Linear RGB, `0..=1`.
    pub color: [f32; 3],
    /// In screen uv (`0, 0` top-left, `1, 1` bottom-right).
    pub center: [f32; 2],
}

impl Vignette {
    /// Unity's floor, short of a hard edge.
    pub const MIN_SMOOTHNESS: f32 = 0.01;
    pub const DEFAULT: Self = Self {
        active: false,
        intensity: 0.3,
        smoothness: 0.2,
        color: [0.0; 3],
        center: [0.5, 0.5],
    };
}

impl Default for Vignette {
    fn default() -> Self {
        Self::DEFAULT
    }
}
