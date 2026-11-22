//! The ops are keyed by `(materials, key)` rather than `&mut Scene` so the inspector, which
//! split-borrows `scene.materials` and `scene.world`, can call them.

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
pub use detail::{
    clamp_detail, detail_mask, set_detail_map, set_detail_mask, DETAIL_FIELDS, DETAIL_SCALE_RANGE,
};
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

pub type MaterialLibrary = BTreeMap<String, MaterialAsset>;

/// Stores `asset` unclamped: `Material.DefineAsset` then runs metallic, roughness, alpha and the
/// modes through their `set_*` ops, never base colour or emissive.
pub fn define_asset(materials: &mut MaterialLibrary, name: &str, asset: MaterialAsset) {
    fit_map_slots(&asset);
    materials.insert(name.to_string(), asset);
}

pub fn ensure_named(materials: &mut MaterialLibrary, name: &str) {
    materials.entry(name.to_string()).or_default();
}

/// Fills the key only when the library has nothing there: an existing authored asset is kept.
pub fn fold_pending(materials: &mut MaterialLibrary, key: &str, pending: MaterialAsset) {
    materials.entry(key.to_string()).or_insert(pending);
}

pub fn ensure_material_key(scene: &mut Scene, id: u32) -> Option<String> {
    scene.ensure_material_key(id)
}

pub fn set_base_color(materials: &mut MaterialLibrary, key: &str, rgb: [f32; 3]) {
    if let Some(m) = materials.get_mut(key) {
        m.base_color = finite_array_or(rgb, m.base_color).map(unit);
    }
}

pub fn set_base_color_map(materials: &mut MaterialLibrary, key: &str, path: String) {
    if let Some(m) = materials.get_mut(key) {
        m.base_color_map = (!path.is_empty()).then(|| MapRef::from(path));
    }
}

pub fn set_metallic(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.metallic = unit(value);
    }
}

pub fn set_metallic_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.metallic_map = path.map(MapRef::from);
        fit_data_slot(m.metallic_map.as_ref());
    }
}

pub fn set_roughness(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness = unit(value);
    }
}

pub fn set_roughness_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness_map = path.map(MapRef::from);
        fit_data_slot(m.roughness_map.as_ref());
    }
}

/// An image still of Texture Type `Default` becomes a `NormalMap`.
pub fn set_normal_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.normal_map = path.map(MapRef::from);
        fit_normal_slot(m.normal_map.as_ref());
    }
}

pub fn set_occlusion_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    set_level_map(
        materials,
        key,
        path,
        |m| &mut m.occlusion_map,
        fit_data_slot,
    );
}

pub fn set_specular_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    set_level_map(materials, key, path, |m| &mut m.specular_map, fit_data_slot);
}

pub fn set_height_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    set_level_map(
        materials,
        key,
        path,
        |m| &mut m.height_map,
        |map| fit_single_channel_slot(map, "a height map"),
    );
}

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

/// Call wherever a material gets its maps whole, as `define_asset`, a maps bake and a model's
/// import do.
pub fn fit_map_slots(asset: &MaterialAsset) {
    fit_normal_slot(asset.normal_map.as_ref());
    fit_data_slot(asset.metallic_map.as_ref());
    fit_data_slot(asset.roughness_map.as_ref());
    fit_data_slot(asset.occlusion_map.as_ref());
    fit_data_slot(asset.specular_map.as_ref());
    fit_single_channel_slot(asset.height_map.as_ref(), "a height map");
    fit_data_slot(asset.detail.as_ref().and_then(|d| d.map.as_ref()));
    fit_single_channel_slot(
        asset.detail.as_ref().and_then(|d| d.mask.as_ref()),
        "a detail mask",
    );
}

pub fn fit_normal_slot(map: Option<&MapRef>) {
    if let Some(file) = map.and_then(MapRef::file) {
        rusty_asset::sidecar::fit_normal_slot(&file);
    }
}

pub fn fit_single_channel_slot(map: Option<&MapRef>, used: &str) {
    if let Some(file) = map.and_then(MapRef::file) {
        rusty_asset::sidecar::fit_single_channel_slot(&file, used);
    }
}

/// A `Default` image still marked sRGB turns it off: its texels are data.
pub fn fit_data_slot(map: Option<&MapRef>) {
    if let Some(file) = map.and_then(MapRef::file) {
        rusty_asset::sidecar::fit_data_slot(&file);
    }
}

pub fn set_emissive(materials: &mut MaterialLibrary, key: &str, rgb: [f32; 3]) {
    if let Some(m) = materials.get_mut(key) {
        m.emissive = finite_array_or(rgb, m.emissive).map(non_negative);
    }
}

pub fn set_emissive_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.emissive_map = path.map(MapRef::from);
    }
}

pub fn set_render_mode(materials: &mut MaterialLibrary, key: &str, mode: RenderMode) {
    if let Some(m) = materials.get_mut(key) {
        m.render_mode = mode;
    }
}

pub fn set_render_face(materials: &mut MaterialLibrary, key: &str, face: RenderFace) {
    if let Some(m) = materials.get_mut(key) {
        m.render_face = face;
    }
}

pub fn set_alpha(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.alpha = unit(value);
    }
}

pub fn set_alpha_cutoff(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.alpha_cutoff = unit(value);
    }
}

#[cfg(test)]
mod tests;
