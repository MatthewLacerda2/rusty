//! src/components/material.rs — Material asset + the entity's reference to it.
//!
//! A *material* is an authored/imported asset (glTF 2.0 metallic-roughness data):
//! its values live ONCE in the per-World material library (`Scene.materials`), and
//! many entities can reference the same one. The entity itself carries only a thin
//! reference — `MaterialComponent { materials: [<library key>, …] }`, one key per
//! sub-mesh of its mesh (#1285) — which IS a first-class component (an
//! engine-provided per-entity type), while the data it points at is the shared asset.

mod decal;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::asset::MapRef;
use crate::procgen::TextureRecipe;
pub use decal::DecalBlend;

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

fn default_true() -> bool {
    true
}

fn is_true(value: &bool) -> bool {
    *value
}

crate::core::named_enum! {
    /// How a material's surface is composited — Unity's "Rendering Mode" (#242). Plain
    /// serde data, mirroring [`ParticleBlend`](super::ParticleBlend) as prior art for a
    /// blend selector. `Opaque` is the default and the fast path.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RenderMode {
        /// Fully opaque (`REPLACE`, depth write on). The default; rides the solids pass.
        #[default]
        Opaque = "Opaque",
        /// Alpha-tested: the fragment is `discard`ed below `alpha_cutoff`, else fully
        /// opaque. Writes depth, needs no sorting, so it also rides the solids pass
        /// (foliage, chain-link, grates).
        Cutout = "Cutout",
        /// Alpha-blended (`ALPHA_BLENDING`), depth test on / write off, drawn in a
        /// separate back-to-front-sorted pass after opaque (glass, holograms, fades).
        Transparent = "Transparent",
    }
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
    pub base_color_map: Option<MapRef>,
    #[serde(default)]
    pub metallic: f32,
    #[serde(default)]
    pub metallic_map: Option<MapRef>,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub roughness_map: Option<MapRef>,
    #[serde(default)]
    pub normal_map: Option<MapRef>,
    #[serde(default)]
    pub emissive: [f32; 3],
    #[serde(default)]
    pub emissive_map: Option<MapRef>,
    /// Compositing mode (#242); `Opaque` when absent.
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
    /// The authored surface shader this material renders with (#396): a module name
    /// resolved as `<name>.wgsl` in `assets/shaders` (where `Shader.Bake`
    /// writes), then `engine/shaders`. `None` is the standard forward shader.
    #[serde(default)]
    pub shader: Option<String>,
    /// The shader's runtime param values (#399), by canonical name
    /// (`"hit_flash.amount"`, see `shadergen::params`): Unity's `material.SetFloat`.
    /// A param not listed draws with its baked default; a name the current shader
    /// does not have is ignored. Empty — and absent from the scene file — by default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shader_params: BTreeMap<String, Vec<f32>>,
    /// The textures the shader's extra slots sample (#400), by slot name
    /// (`"mask"`, see `shadergen::textures`) → texture path (a PNG, or an
    /// `rt:<name>` render texture). A slot not named — or whose file is missing —
    /// samples white. Empty — and absent from the scene file — by default.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shader_textures: BTreeMap<String, MapRef>,
    /// The multi-output texture recipe the maps bake from (#409) — the `maps` key of
    /// the document. The recipe is the truth and the baked PNGs a regenerable cache
    /// (`Material.Rebake`). `None` — and absent from the scene file — by default.
    #[serde(default, rename = "maps", skip_serializing_if = "Option::is_none")]
    pub maps_recipe: Option<TextureRecipe>,
    /// Whether surfaces drawn with this material take world decals (#638), HDRP's
    /// "Receive Decals". Turn it off for characters and props that move through
    /// bullet holes. Transparent materials never receive decals. Defaults to `true`
    /// and is written to the scene file only when off.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub receive_decals: bool,
    /// What this material changes when a decal stamps it (#638): per-channel blend
    /// weights and the angle fade. Read only by `Decals.Spawn { material = … }`;
    /// absent from the scene file while it holds the defaults.
    #[serde(default, skip_serializing_if = "DecalBlend::is_default")]
    pub decal: DecalBlend,
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
            shader: None,
            shader_params: BTreeMap::new(),
            shader_textures: BTreeMap::new(),
            maps_recipe: None,
            receive_decals: true,
            decal: DecalBlend::default(),
        }
    }
}

impl MaterialAsset {
    /// Whether this material is drawn in the alpha-blended transparent pass (#242)
    /// rather than the opaque solids pass. `Opaque`/`Cutout` both ride the solids
    /// pass; only `Transparent` defers to the sorted pass.
    pub fn is_transparent(&self) -> bool {
        self.render_mode == RenderMode::Transparent
    }
}

/// The entity's references to library materials — the first-class component. Holds
/// only library keys; the data lives in `Scene.materials`.
///
/// One key per sub-mesh of the entity's mesh, in the mesh's range order (Unity's
/// `Renderer.sharedMaterials`, #1285): a mesh imported with two materials draws its
/// first index range with `materials[0]` and its second with `materials[1]`. A
/// one-range mesh has a one-entry list. As in Unity, a range past the end of the list
/// is not drawn, and a material past the last range draws that range again over it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialComponent {
    pub materials: Vec<String>,
}

impl MaterialComponent {
    /// A one-material reference to library key `key`.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            materials: vec![key.into()],
        }
    }

    /// The reference to entity `id`'s own library material, keyed
    /// [`entity_material_key`] — what every "give this entity a material" path attaches.
    pub fn for_entity(id: u32) -> Self {
        Self::new(entity_material_key(id))
    }

    /// The first material's key (Unity's `Renderer.sharedMaterial`), or `""` for an
    /// empty list, which no library entry has.
    pub fn first(&self) -> &str {
        self.materials.first().map_or("", String::as_str)
    }
}

/// The library key of entity `id`'s own material (`entity_{id}_material`): the one
/// spelling every surface that creates a per-entity material uses (#997).
pub fn entity_material_key(id: u32) -> String {
    format!("entity_{id}_material")
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
    fn a_material_without_transparency_fields_loads_opaque() {
        // A hand-written material that names no render_mode/alpha/alpha_cutoff loads
        // through `#[serde(default)]` as a fully-opaque material.
        let json = r#"{ "base_color": [0.5, 0.5, 0.5], "metallic": 0.0, "roughness": 0.5 }"#;
        let m: MaterialAsset = serde_json::from_str(json).expect("a sparse material parses");
        assert_eq!(m.render_mode, RenderMode::Opaque);
        assert_eq!(m.alpha, 1.0);
        assert_eq!(m.alpha_cutoff, 0.5);
    }

    #[test]
    fn shader_params_and_textures_round_trip_and_are_omitted_when_empty() {
        let m = MaterialAsset {
            shader: Some("enemy_hit".into()),
            shader_params: BTreeMap::from([("hit_flash.amount".into(), vec![0.5])]),
            shader_textures: BTreeMap::from([("mask".into(), "noise.png".into())]),
            ..MaterialAsset::default()
        };
        let back: MaterialAsset =
            serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back.shader_params, m.shader_params);
        assert_eq!(back.shader_textures, m.shader_textures);
        let plain = serde_json::to_string(&MaterialAsset::default()).unwrap();
        assert!(!plain.contains("shader_params"), "{plain}");
        assert!(!plain.contains("shader_textures"), "{plain}");
    }
}
