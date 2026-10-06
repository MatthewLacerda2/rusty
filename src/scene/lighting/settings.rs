//! The scene's lighting settings (#832): what Generate Lighting bakes with, stored
//! with the scene the way Unity keeps a Lighting Settings asset per scene, so a
//! rebake never asks for the knobs again.

use serde::{Deserialize, Serialize};

use super::lightmap::BakeSettings;

/// Unity's Lighting Settings, the part rusty has: the lightmap bake's knobs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LightingSettings {
    /// Texels per unit, samples, bounces, seed, filter and the directional mode.
    pub lightmaps: BakeSettings,
}

impl LightingSettings {
    /// True when every knob is at its default: the scene file then leaves the block
    /// out, so a scene that never touched them saves byte for byte as before.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
