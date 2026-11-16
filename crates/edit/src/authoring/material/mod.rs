//! crates/edit/src/authoring/material/mod.rs — Shared material-authoring ops.
//!
//! The ONE place the engine knows how to mutate a library `MaterialAsset` field by
//! field (the PBR factors, the glTF-PBR map paths, and the transparency story —
//! render mode + alpha + cutoff, #242), including the validation that belongs to
//! each write (the [`FACTOR_RANGE`] clamp on metallic, roughness, alpha and cutoff,
//! which the editor's sliders read too, #993).
//!
//! BOTH callers route through here so the editor and the API can never drift: the
//! inspector's Material card (`editor::inspector::components::material`) and the Lua
//! `Material.*` namespace (`api::material`) are siblings over these same Rust ops —
//! the egui panel and the Lua binding mutate the asset through one implementation, so
//! the field write + validation lives once (#287). The ops are keyed by
//! `(materials, key)` rather than `&mut Scene` so the inspector — which split-borrows
//! `scene.materials` and `scene.world` at once — can call them without a borrow
//! conflict.
//!
//! Pure: no wall-clock, no RNG.

mod coordinates;
mod detail;
mod face;
mod field;
mod fonts;
mod graphs;
mod library;
mod maps;
mod shader_params;
mod slots;
mod sources;
mod surface;

use std::collections::BTreeMap;

pub use rusty_components::material::entity_material_key;

pub use coordinates::{
    BLEND_SHARPNESS_RANGE, COORDINATE_FIELDS, HEIGHT_SCALE_RANGE, MAP_COORD_RANGE, WORLD_SIZE_RANGE,
};
pub use detail::{clamp_detail, set_detail_map, DETAIL_FIELDS, DETAIL_SCALE_RANGE};
pub use face::FACE_FIELDS;
pub use field::{
    material_fields, set_surface, surface_field, SurfaceField, SurfaceKind, SurfaceValue,
};
pub use graphs::{engine_graphs, listed_graphs, project_graphs, read_graph};
pub use library::{
    delete_asset, describe, describe_all, drop_unused, entity_keys, restore_asset, users,
    DeletedMaterial, MaterialUser,
};
pub use maps::{bake_maps, rebake_maps, MAPS_DIR};
pub use shader_params::{
    clear_entity_shader_param, entity_shader_param, entity_shader_param_with,
    set_entity_shader_param, set_entity_shader_param_with, set_shader, set_shader_param,
    set_shader_texture, shader_layout, shader_param,
};
pub use slots::{keys, set_slot, slot_count};
pub use sources::{load_recipe, Loaded};
pub use surface::{import_field, import_reflectance, SURFACE_FIELDS};

use rusty_asset::MapRef;
use rusty_components::RenderFace;
use rusty_scene::rules::sanitize::{finite_array_or, non_negative, unit};
use rusty_scene::{MaterialAsset, RenderMode, Scene};

pub use rusty_scene::rules::material::FACTOR_RANGE;

/// The scene's material library: library key -> shared asset. Matches
/// `Scene.materials`; aliased here so callers name one type.
pub type MaterialLibrary = BTreeMap<String, MaterialAsset>;

/// Define a named library asset, inserting (or overwriting) `asset` under `name`. The
/// single insert-by-name both the `Material.DefineAsset` binding and any future editor
/// "new material asset" entry use, so a standalone material is authored one way. The
/// asset's own fields are still validated by routing each value through the per-field
/// `set_*` ops (the API does this), so the clamps stay single-sourced.
pub fn define_asset(materials: &mut MaterialLibrary, name: &str, asset: MaterialAsset) {
    fit_map_slots(&asset);
    materials.insert(name.to_string(), asset);
}

/// Ensure a named library asset exists, inserting a default one if absent, and return
/// `name` for chaining the per-field `set_*` ops. Unlike [`define_asset`] this never
/// overwrites an existing asset — it is the resolve-or-create the per-field authoring
/// path uses when building an asset up field by field under a chosen name.
pub fn ensure_named(materials: &mut MaterialLibrary, name: &str) {
    materials.entry(name.to_string()).or_default();
}

/// Fold a staged material (the Add menu's default, which lacked library access) into
/// the library under `key`. It fills the key only when the library has nothing there:
/// an existing authored asset is kept, the rule every attach path shares (#997). The
/// inspector's card and a Play/Stop restore (`apply_world_data`) both fold through it.
pub fn fold_pending(materials: &mut MaterialLibrary, key: &str, pending: MaterialAsset) {
    materials.entry(key.to_string()).or_insert(pending);
}

/// Resolve the library key for entity `id`'s first material, creating a default
/// one if it has none: [`Scene::ensure_material_key`]. `None` only when the entity
/// does not exist. The API's setters resolve through it before applying an op.
pub fn ensure_material_key(scene: &mut Scene, id: u32) -> Option<String> {
    scene.ensure_material_key(id)
}

/// Set the base-color (albedo tint) factor.
pub fn set_base_color(materials: &mut MaterialLibrary, key: &str, rgb: [f32; 3]) {
    if let Some(m) = materials.get_mut(key) {
        m.base_color = finite_array_or(rgb, m.base_color).map(unit);
    }
}

/// Set the base-color (albedo) map path; an empty path clears it to `None`.
pub fn set_base_color_map(materials: &mut MaterialLibrary, key: &str, path: String) {
    if let Some(m) = materials.get_mut(key) {
        m.base_color_map = (!path.is_empty()).then(|| MapRef::from(path));
    }
}

/// Set the metallic scalar factor, clamped to [`FACTOR_RANGE`].
pub fn set_metallic(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.metallic = unit(value);
    }
}

/// Set (or, with `None`, clear) the metallic map path. A `Default` image still
/// marked sRGB turns it off (`sidecar::fit_data_slot`, #1574).
pub fn set_metallic_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.metallic_map = path.map(MapRef::from);
        fit_data_slot(m.metallic_map.as_ref());
    }
}

/// Set the roughness scalar factor, clamped to [`FACTOR_RANGE`].
pub fn set_roughness(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness = unit(value);
    }
}

/// Set (or, with `None`, clear) the roughness map path. A `Default` image still
/// marked sRGB turns it off (`sidecar::fit_data_slot`, #1574).
pub fn set_roughness_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness_map = path.map(MapRef::from);
        fit_data_slot(m.roughness_map.as_ref());
    }
}

/// Set (or, with `None`, clear) the normal map path. An image still of Texture
/// Type `Default` becomes a `NormalMap` (`sidecar::fit_normal_slot`, #1574).
pub fn set_normal_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.normal_map = path.map(MapRef::from);
        fit_normal_slot(m.normal_map.as_ref());
    }
}

/// Set (or, with `None` or an empty path, clear) the occlusion map path (#2151);
/// a data map, as metallic's. A map that darkens is never left naming nothing.
pub fn set_occlusion_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    set_level_map(
        materials,
        key,
        path,
        |m| &mut m.occlusion_map,
        fit_data_slot,
    );
}

/// Set (or, with `None` or an empty path, clear) the specular-level map path
/// (#2151); a data map. A map that scales is never left naming nothing.
pub fn set_specular_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    set_level_map(materials, key, path, |m| &mut m.specular_map, fit_data_slot);
}

/// Set (or, with `None` or an empty path, clear) the height map path (#2191); a
/// `Default` image becomes `SingleChannel` ([`fit_height_slot`]). A map that
/// displaces is never left naming nothing.
pub fn set_height_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    set_level_map(materials, key, path, |m| &mut m.height_map, fit_height_slot);
}

/// Write a data map that darkens, scales or displaces into the `field` of `key`,
/// an empty path clearing it, and `fit` its image's import settings.
fn set_level_map(
    materials: &mut MaterialLibrary,
    key: &str,
    path: Option<String>,
    field: fn(&mut MaterialAsset) -> &mut Option<MapRef>,
    fit: fn(Option<&MapRef>),
) {
    if let Some(m) = materials.get_mut(key) {
        let slot = field(m);
        *slot = path.filter(|p| !p.is_empty()).map(MapRef::from);
        fit(slot.as_ref());
    }
}

/// Fit the import settings of every image `asset`'s slots name to the slot:
/// [`fit_normal_slot`], [`fit_data_slot`] and [`fit_height_slot`]. Every way a material gets its maps
/// whole goes through it (a material document, a maps recipe, a model's import).
pub fn fit_map_slots(asset: &MaterialAsset) {
    fit_normal_slot(asset.normal_map.as_ref());
    fit_data_slot(asset.metallic_map.as_ref());
    fit_data_slot(asset.roughness_map.as_ref());
    fit_data_slot(asset.occlusion_map.as_ref());
    fit_data_slot(asset.specular_map.as_ref());
    fit_height_slot(asset.height_map.as_ref());
    fit_data_slot(asset.detail.as_ref().and_then(|d| d.map.as_ref()));
}

/// [`rusty_asset::sidecar::fit_normal_slot`] for the image a normal slot names.
pub fn fit_normal_slot(map: Option<&MapRef>) {
    if let Some(file) = map.and_then(MapRef::file) {
        rusty_asset::sidecar::fit_normal_slot(&file);
    }
}

/// [`rusty_asset::sidecar::fit_height_slot`] for the image a height slot names.
pub fn fit_height_slot(map: Option<&MapRef>) {
    if let Some(file) = map.and_then(MapRef::file) {
        rusty_asset::sidecar::fit_height_slot(&file);
    }
}

/// [`rusty_asset::sidecar::fit_data_slot`] for the image a data slot (metallic,
/// roughness, occlusion, specular level) names.
pub fn fit_data_slot(map: Option<&MapRef>) {
    if let Some(file) = map.and_then(MapRef::file) {
        rusty_asset::sidecar::fit_data_slot(&file);
    }
}

/// Set the emissive factor.
pub fn set_emissive(materials: &mut MaterialLibrary, key: &str, rgb: [f32; 3]) {
    if let Some(m) = materials.get_mut(key) {
        m.emissive = finite_array_or(rgb, m.emissive).map(non_negative);
    }
}

/// Set (or, with `None`, clear) the emissive map path.
pub fn set_emissive_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.emissive_map = path.map(MapRef::from);
    }
}

/// Set the compositing/render mode (typed — string parsing stays in the API adapter).
pub fn set_render_mode(materials: &mut MaterialLibrary, key: &str, mode: RenderMode) {
    if let Some(m) = materials.get_mut(key) {
        m.render_mode = mode;
    }
}

/// Set which faces the material draws (#1615): the typed form of the `render_face`
/// row ([`FACE_FIELDS`]) that `Material.SetRenderFace` and the Inspector write.
pub fn set_render_face(materials: &mut MaterialLibrary, key: &str, face: RenderFace) {
    if let Some(m) = materials.get_mut(key) {
        m.render_face = face;
    }
}

/// Set the base-color alpha, clamped to [`FACTOR_RANGE`]. The clamp validation lives
/// HERE, once — both the editor slider and the `Material.SetAlpha` binding inherit it.
pub fn set_alpha(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.alpha = unit(value);
    }
}

/// Set the alpha-test cutoff, clamped to [`FACTOR_RANGE`] (the single source of that
/// validation, shared by the editor slider and `Material.SetAlphaCutoff`).
pub fn set_alpha_cutoff(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.alpha_cutoff = unit(value);
    }
}

#[cfg(test)]
mod tests;
