//! Bloom, Unity URP's override of that name: bright parts of the HDR scene bleed
//! light into their surroundings.

use serde::{Deserialize, Serialize};

/// Whether bloom runs, how strong it is and which brightness blooms.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bloom {
    pub active: bool,
    /// How much of the blurred bright pass is added back, `≥ 0`.
    pub intensity: f32,
    /// The luminance above which a pixel blooms, `≥ 0`.
    pub threshold: f32,
}

impl Default for Bloom {
    /// On, at full strength, from a luminance of `0.8`.
    fn default() -> Self {
        Self {
            active: true,
            intensity: 1.0,
            threshold: 0.8,
        }
    }
}
