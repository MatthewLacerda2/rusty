//! src/components/texture.rs — Texture/material component
//!
//! albedo/metallic/roughness/color. Moved verbatim from the legacy
//! `core/scene.rs`.

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
