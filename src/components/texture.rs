//! src/components/texture.rs — LEGACY inline material component.
//!
//! Pre-#201 the material lived inline on each entity as this `TextureComponent`
//! (albedo/metallic/roughness/color). It is now retained ONLY for back-compat
//! deserialization of old scenes: `MaterialAsset::from_legacy` migrates one into
//! the per-World material library, and entities reference materials by name via
//! `MaterialComponent`. Do not add new uses.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextureComponent {
    pub path: String,
    pub is_dirty: bool,
    #[serde(default)]
    pub metallic: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub metallic_map: Option<String>,
    #[serde(default)]
    pub roughness_map: Option<String>,
    #[serde(default = "default_color")]
    pub color: [f32; 3],
}

fn default_roughness() -> f32 {
    0.5
}

fn default_color() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}
