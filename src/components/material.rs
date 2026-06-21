//! src/components/material.rs — Material asset + the entity's reference to it.
//!
//! A *material* is an authored/imported asset (glTF 2.0 metallic-roughness data):
//! its values live ONCE in the per-World material library (`Scene.materials`), and
//! many entities can reference the same one. The entity itself carries only a thin
//! reference — `MaterialComponent { material: <library key> }` — which IS a
//! first-class component (an engine-provided per-entity type), while the data it
//! points at is the shared asset.
//!
//! `TextureComponent` (legacy, in `texture.rs`) is kept solely for back-compat
//! deserialization of pre-#201 scenes that stored the material inline per entity;
//! `MaterialAsset::from_legacy` migrates one across.

use serde::{Deserialize, Serialize};

use super::TextureComponent;

fn default_base_color() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn default_roughness() -> f32 {
    0.5
}

/// The glTF 2.0 metallic-roughness material data — a reusable asset. Stored in the
/// scene's material library and referenced by name from entities. Plain serde data,
/// never any GPU buffers.
///
/// NOTE: every field here is sampled by the renderer — `base_color`/`base_color_map`
/// plus `metallic`/`roughness` (#201), the `metallic_map`/`roughness_map` (#202), the flat
/// `emissive` factor (#222), and the `normal_map`/`emissive_map` (#207 — the normal map
/// via a per-vertex tangent attribute + TBN in `fs_main`). No field is a write-only no-op.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialAsset {
    #[serde(default = "default_base_color")]
    pub base_color: [f32; 3],
    #[serde(default)]
    pub base_color_map: Option<String>,
    #[serde(default)]
    pub metallic: f32,
    #[serde(default)]
    pub metallic_map: Option<String>,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub roughness_map: Option<String>,
    #[serde(default)]
    pub normal_map: Option<String>,
    #[serde(default)]
    pub emissive: [f32; 3],
    #[serde(default)]
    pub emissive_map: Option<String>,
}

impl Default for MaterialAsset {
    fn default() -> Self {
        Self {
            base_color: default_base_color(),
            base_color_map: None,
            metallic: 0.0,
            metallic_map: None,
            roughness: default_roughness(),
            roughness_map: None,
            normal_map: None,
            emissive: [0.0, 0.0, 0.0],
            emissive_map: None,
        }
    }
}

impl MaterialAsset {
    /// Migrate a legacy inline `TextureComponent` into a `MaterialAsset`. An empty
    /// albedo path maps to `None`; normal/emissive default (the legacy type had
    /// neither). Drops the legacy `is_dirty` flag (write-only, never read).
    pub fn from_legacy(t: &TextureComponent) -> Self {
        Self {
            base_color: t.color,
            base_color_map: (!t.path.is_empty()).then(|| t.path.clone()),
            metallic: t.metallic,
            metallic_map: t.metallic_map.clone(),
            roughness: t.roughness,
            roughness_map: t.roughness_map.clone(),
            normal_map: None,
            emissive: [0.0, 0.0, 0.0],
            emissive_map: None,
        }
    }
}

/// The entity's reference to a library material — the first-class component. Holds
/// only the library key/name; the data lives in `Scene.materials`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialComponent {
    pub material: String,
}
