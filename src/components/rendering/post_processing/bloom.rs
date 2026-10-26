//! Bloom, Unity URP's override of that name: bright parts of the HDR scene bleed
//! light into their surroundings, and, with a dirt texture, light up the smudges on
//! the lens (#1442).

use serde::{Deserialize, Serialize};

/// The engine's own lens dirt (#1442), a neutral smudge texture procedurally
/// generated for when a game has no art of its own: name it as the dirt texture.
pub const DEFAULT_DIRT_TEXTURE: &str = "engine/textures/lens_dirt.png";

/// Whether bloom runs, how strong it is, which brightness blooms and the lens dirt
/// it lights.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bloom {
    pub active: bool,
    /// How much of the blurred bright pass is added back, `≥ 0`.
    pub intensity: f32,
    /// The luminance above which a pixel blooms, `≥ 0`.
    pub threshold: f32,
    /// The lens dirt texture, a project-relative path (or
    /// [`DEFAULT_DIRT_TEXTURE`]), stretched over the screen and lit by the bloom;
    /// none by default.
    pub dirt_texture: Option<String>,
    /// How much of the bloom the dirt adds back, `≥ 0`.
    pub dirt_intensity: f32,
}

impl Default for Bloom {
    /// On, at full strength, from a luminance of `0.8`; no dirt, as in Unity.
    fn default() -> Self {
        Self {
            active: true,
            intensity: 1.0,
            threshold: 0.8,
            dirt_texture: None,
            dirt_intensity: 0.0,
        }
    }
}
