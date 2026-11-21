use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromaticAberration {
    pub active: bool,
    /// At the screen's corners, `0..=1`.
    pub intensity: f32,
}

impl ChromaticAberration {
    pub const DEFAULT: Self = Self {
        active: false,
        intensity: 0.3,
    };
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        Self::DEFAULT
    }
}
