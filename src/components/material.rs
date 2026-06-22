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

fn default_alpha() -> f32 {
    1.0
}

fn default_alpha_cutoff() -> f32 {
    0.5
}

/// How a material's surface is composited — Unity's "Rendering Mode" (#242). Plain
/// serde data, mirroring [`ParticleBlend`](super::ParticleBlend) as prior art for a
/// blend selector. `Opaque` is the default and the fast path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    /// Fully opaque (`REPLACE`, depth write on). The default; rides the solids pass.
    #[default]
    Opaque,
    /// Alpha-tested: the fragment is `discard`ed below `alpha_cutoff`, else fully
    /// opaque. Writes depth, needs no sorting, so it also rides the solids pass
    /// (foliage, chain-link, grates).
    Cutout,
    /// Alpha-blended (`ALPHA_BLENDING`), depth test on / write off, drawn in a
    /// separate back-to-front-sorted pass after opaque (glass, holograms, fades).
    Transparent,
}

/// The glTF 2.0 metallic-roughness material data — a reusable asset. Stored in the
/// scene's material library and referenced by name from entities. Plain serde data,
/// never any GPU buffers.
///
/// NOTE: every field here is sampled by the renderer — `base_color`/`base_color_map`
/// plus `metallic`/`roughness` (#201), the `metallic_map`/`roughness_map` (#202), the flat
/// `emissive` factor (#222), and the `normal_map`/`emissive_map` (#207 — the normal map
/// via a per-vertex tangent attribute + TBN in `fs_main`). The transparency story —
/// `render_mode` + `alpha` + `alpha_cutoff` (#242) — drives blend/discard. No field is a
/// write-only no-op.
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
    /// Compositing mode (#242). `#[serde(default)]` => `Opaque`, so pre-#242 scenes
    /// load unchanged.
    #[serde(default)]
    pub render_mode: RenderMode,
    /// Base-color alpha in `[0, 1]`. Only read when `render_mode == Transparent`
    /// (the blend factor); `Opaque`/`Cutout` ignore it. Defaults to 1.0 (fully
    /// opaque) so existing materials are unaffected.
    #[serde(default = "default_alpha")]
    pub alpha: f32,
    /// Alpha-test threshold in `[0, 1]` for `Cutout`: fragments whose sampled alpha
    /// is below this are discarded. Defaults to 0.5 (Unity's default).
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
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
            render_mode: RenderMode::Opaque,
            alpha: default_alpha(),
            alpha_cutoff: default_alpha_cutoff(),
        }
    }
}

impl MaterialAsset {
    /// Migrate a legacy inline `TextureComponent` into a `MaterialAsset`. An empty
    /// albedo path maps to `None`; normal/emissive default (the legacy type had
    /// neither). Drops the legacy `is_dirty` flag (write-only, never read). Legacy
    /// materials were always opaque, so the transparency fields take their defaults.
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
            render_mode: RenderMode::Opaque,
            alpha: default_alpha(),
            alpha_cutoff: default_alpha_cutoff(),
        }
    }

    /// Whether this material is drawn in the alpha-blended transparent pass (#242)
    /// rather than the opaque solids pass. `Opaque`/`Cutout` both ride the solids
    /// pass; only `Transparent` defers to the sorted pass.
    pub fn is_transparent(&self) -> bool {
        self.render_mode == RenderMode::Transparent
    }
}

/// The entity's reference to a library material — the first-class component. Holds
/// only the library key/name; the data lives in `Scene.materials`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialComponent {
    pub material: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_opaque_and_fully_solid() {
        let m = MaterialAsset::default();
        assert_eq!(m.render_mode, RenderMode::Opaque);
        assert_eq!(m.alpha, 1.0);
        assert_eq!(m.alpha_cutoff, 0.5);
        assert!(!m.is_transparent());
    }

    #[test]
    fn is_transparent_only_for_transparent_mode() {
        let with_mode = |mode| MaterialAsset {
            render_mode: mode,
            ..MaterialAsset::default()
        };
        assert!(!with_mode(RenderMode::Opaque).is_transparent());
        assert!(!with_mode(RenderMode::Cutout).is_transparent());
        assert!(with_mode(RenderMode::Transparent).is_transparent());
    }

    #[test]
    fn transparency_fields_round_trip_through_serde() {
        let m = MaterialAsset {
            render_mode: RenderMode::Transparent,
            alpha: 0.25,
            alpha_cutoff: 0.7,
            ..MaterialAsset::default()
        };
        let json = serde_json::to_string(&m).expect("serialize");
        let back: MaterialAsset = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.render_mode, RenderMode::Transparent);
        assert_eq!(back.alpha, 0.25);
        assert_eq!(back.alpha_cutoff, 0.7);
    }

    #[test]
    fn pre_242_material_without_transparency_fields_loads_opaque() {
        // A material JSON predating #242 (no render_mode/alpha/alpha_cutoff) must load
        // via `#[serde(default)]` as a fully-opaque material, unchanged.
        let json = r#"{ "base_color": [0.5, 0.5, 0.5], "metallic": 0.0, "roughness": 0.5 }"#;
        let m: MaterialAsset = serde_json::from_str(json).expect("legacy material parses");
        assert_eq!(m.render_mode, RenderMode::Opaque);
        assert_eq!(m.alpha, 1.0);
        assert_eq!(m.alpha_cutoff, 0.5);
    }
}
