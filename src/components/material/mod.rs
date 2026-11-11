//! src/components/material/mod.rs — Material asset + the entity's reference to it.
//!
//! A *material* is an authored/imported asset (glTF 2.0 metallic-roughness data):
//! its values live ONCE in the per-World material library (`Scene.materials`), and
//! many entities can reference the same one. The entity itself carries only a thin
//! reference — `MaterialComponent { materials: [<library key>, …] }`, one key per
//! sub-mesh of its mesh (#1285) — which IS a first-class component (an
//! engine-provided per-entity type), while the data it points at is the shared asset.

mod component;
mod decal;
mod defaults;
mod detail;
mod mapping;
mod surface;
mod temporal;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::asset::MapRef;
use crate::procgen::TextureRecipe;
pub use component::{entity_material_key, MaterialComponent};
pub use decal::DecalBlend;
pub use detail::DetailLayer;
pub use mapping::UvMapping;
use mapping::{default_blend_sharpness, default_tiling};
use surface::{default_ior, default_one, white};
pub use surface::{BlendMode, RefractionModel, RenderFace, RenderMode};
pub use temporal::{TaaReactive, MAX_REACTIVE};

/// How far a material's [`MaterialAsset::sorting_priority`] reaches either way:
/// HDRP's `HDRenderQueue.sortingPriorityRange`.
pub const SORTING_PRIORITY_RANGE: i32 = 50;

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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Unity's *Tiling* (#1573): how many times the maps above repeat across UV0,
    /// or across `world_size` metres under a world mapping. (1, 1) by default.
    #[serde(default = "default_tiling")]
    pub tiling: [f32; 2],
    /// Unity's *Offset*, added after `tiling`, in texture repeats. (0, 0) by default.
    #[serde(default)]
    pub offset: [f32; 2],
    /// Where the maps are read, HDRP's *Base UV Mapping*: UV0 (the default), or
    /// projected from world space.
    #[serde(default)]
    pub uv_mapping: UvMapping,
    /// How many metres one repeat covers under a world mapping (HDRP's *World
    /// Scale* is its inverse). 1 by default.
    #[serde(default = "default_one")]
    pub world_size: f32,
    /// How sharply `Triplanar` turns from one projection to the next: the normal's
    /// components are raised to this power before they weight the three. 4 by default.
    #[serde(default = "default_blend_sharpness")]
    pub blend_sharpness: f32,
    /// HDRP Lit's detail map (#1997, [`DetailLayer`]); `None` by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<DetailLayer>,
    /// Compositing mode (#242); `Opaque` when absent.
    #[serde(default)]
    pub render_mode: RenderMode,
    /// Which faces draw (#1615), URP's *Render Face*; `Front` when absent.
    #[serde(default)]
    pub render_face: RenderFace,
    /// Base-color alpha in `[0, 1]`. Only read when `render_mode == Transparent`
    /// (the blend factor); `Opaque`/`Cutout` ignore it. Defaults to 1.0 (fully
    /// opaque) so existing materials are unaffected.
    #[serde(default = "default_alpha")]
    pub alpha: f32,
    /// Alpha-test threshold in `[0, 1]` for `Cutout`: fragments whose sampled alpha
    /// is below this are discarded. Defaults to 0.5 (Unity's default).
    #[serde(default = "default_alpha_cutoff")]
    pub alpha_cutoff: f32,
    /// How a `Transparent` surface combines with what is behind it (#1523), URP's
    /// *Blending Mode*; `Alpha` when absent.
    #[serde(default)]
    pub blend_mode: BlendMode,
    /// URP's *Preserve Specular Lighting* (#1523): a `Transparent` surface's
    /// reflections and highlights are not faded by `alpha`, and its opacity rises
    /// toward grazing angles as its Fresnel reflection does. On by default; ignored
    /// under `Premultiply` and `Multiply`, as URP ignores it.
    #[serde(default = "default_true")]
    pub preserve_specular: bool,
    /// Whether shadows land on this surface (#1523), URP's *Receive Shadows*: off,
    /// the sun's cascades and point/spot shadows skip it (water under a wall's
    /// shadow). On by default.
    #[serde(default = "default_true")]
    pub receive_shadows: bool,
    /// How a `Transparent` surface bends what is behind it (#1523), HDRP's
    /// *Refraction Model*; `None` when absent. Bends by the shading normal, its
    /// normal map included.
    #[serde(default)]
    pub refraction_model: RefractionModel,
    /// HDRP's *Sorting Priority* (#2039): where a `Transparent` surface draws among
    /// the others. Its draws are ordered by it first, a lower one drawing earlier
    /// and a higher one landing on top whatever their depths, then back to front;
    /// in `[-SORTING_PRIORITY_RANGE, SORTING_PRIORITY_RANGE]`, 0 by default
    /// ([`MaterialAsset::sort_priority`]).
    #[serde(default)]
    pub sorting_priority: i32,
    /// The index of refraction (water 1.33, glass 1.5, the default): HDRP's *Index
    /// of Refraction* for a refracting surface's bend, and glTF's
    /// `KHR_materials_ior` for every dielectric's reflectance (#1998,
    /// [`MaterialAsset::dielectric_f0`]).
    #[serde(default = "default_ior")]
    pub ior: f32,
    /// glTF's `KHR_materials_specular` strength in `[0, 1]` (#1998): scales a
    /// dielectric's reflectance head-on and at grazing angles (its F90,
    /// [`MaterialAsset::dielectric_f90`]); 1 (the default) keeps the one `ior`
    /// gives, 0 reflects nothing at any angle.
    #[serde(default = "default_one")]
    pub specular: f32,
    /// glTF's `KHR_materials_specular` colour (#1998): tints a dielectric's
    /// reflectance, and above 1 raises it (a single-quad pane standing for two
    /// surfaces at 2). White by default.
    #[serde(default = "white")]
    pub specular_color: [f32; 3],
    /// How thick a `Planar` refraction's slab is, in metres (default 1).
    #[serde(default = "default_one")]
    pub thickness: f32,
    /// The colour light keeps after `absorption_distance` metres through a
    /// `Transparent` surface (#1523), HDRP's *Transmittance Color*; white (the
    /// default) absorbs nothing. Shallow water stays clear, deep water takes it.
    #[serde(default = "white")]
    pub transmittance_color: [f32; 3],
    /// The distance at which light keeps `transmittance_color`, in metres, HDRP's
    /// *Absorption Distance* (default 1).
    #[serde(default = "default_one")]
    pub absorption_distance: f32,
    /// HDRP's *Transparent Depth Postpass* (#1523): a `Transparent` surface writes
    /// its depth after drawing, so depth of field, motion blur and soft particles
    /// see it rather than what is behind it. Off by default.
    #[serde(default)]
    pub depth_postpass: bool,
    /// HDRP's *Transparent Writes Motion Vectors* (#1845): a `Transparent` surface
    /// writes its own motion (and view depth) into the velocity target, without
    /// writing depth, so TAA, motion blur and depth of field move it as a solid
    /// rather than as what is behind it. For solid-looking translucents (a plasma
    /// ball); off by default.
    #[serde(default)]
    pub writes_motion: bool,
    /// How reactive the surface is under TAA (#1845): `Auto` by default.
    #[serde(default)]
    pub reactive: TaaReactive,
    /// HDRP's *Receive SSR* and *Receive SSR Transparent* (#1570, #2004): whether
    /// the surface takes screen-space reflections where the view runs SSR. An opaque
    /// or cutout surface with it off keeps its probe reflection; a `Transparent` one
    /// with it on traces its own over its probe. `None` (unset) is HDRP's default
    /// for the render mode: on for opaque and cutout, off for transparent
    /// ([`MaterialAsset::receives_ssr`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_ssr: Option<bool>,
    /// The colour thin translucency lets through (#1523), HDRP's *Transmission
    /// Tint*; white by default.
    #[serde(default = "white")]
    pub transmission_color: [f32; 3],
    /// How much light passes through the surface from behind, `[0, 1]`: HDRP's
    /// thin *Translucent* material (a leaf, a lampshade, a banner lit from behind),
    /// opaque or transparent. 0 (the default) lets none through.
    #[serde(default)]
    pub transmission: f32,
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

impl MaterialAsset {
    /// Whether this material is drawn in the alpha-blended transparent pass (#242)
    /// rather than the opaque solids pass. `Opaque`/`Cutout` both ride the solids
    /// pass; only `Transparent` defers to the sorted pass.
    pub fn is_transparent(&self) -> bool {
        self.render_mode == RenderMode::Transparent
    }

    /// Its [`MaterialAsset::sorting_priority`] clamped into HDRP's range, as the
    /// transparent sort reads it: a scene file's out-of-range value is held there.
    pub fn sort_priority(&self) -> i32 {
        let range = SORTING_PRIORITY_RANGE;
        self.sorting_priority.clamp(-range, range)
    }

    /// Whether the surface takes SSR: its `receive_ssr`, or unset, HDRP's default
    /// for its render mode (on unless `Transparent`, where a trace costs a march per
    /// pixel).
    pub fn receives_ssr(&self) -> bool {
        self.receive_ssr.unwrap_or(!self.is_transparent())
    }
}

#[cfg(test)]
mod tests;
