//! Chromatic Aberration (#1439), Unity's override of that name: run on the HDR
//! image before the grade (`engine/shaders/postfx/effects/chromatic_aberration.wgsl`).

use serde::{Deserialize, Serialize};

/// Splits the colour channels towards the screen edges, as a cheap lens does,
/// Unity's *Chromatic Aberration*: radial, nothing at the centre.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromaticAberration {
    /// Whether the fringes draw at all.
    pub active: bool,
    /// How far the channels split at the screen's corners, `0..=1`.
    pub intensity: f32,
}

impl ChromaticAberration {
    /// Off; once on, a third-strength split.
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
