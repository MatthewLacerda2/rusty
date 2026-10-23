//! src/scene/authoring/material/mod.rs — Shared material-authoring ops.
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

mod maps;
mod shader_params;
mod slots;

use std::collections::BTreeMap;
use std::ops::RangeInclusive;

pub use crate::components::material::entity_material_key;

pub use maps::{bake_maps, rebake_maps, MAPS_DIR};
pub use shader_params::{
    clear_entity_shader_param, entity_shader_param, entity_shader_param_with,
    set_entity_shader_param, set_entity_shader_param_with, set_shader_param, shader_layout,
    shader_param,
};
pub use slots::{keys, set_slot, slot_count};

use super::sanitize::unit;
use crate::asset::MapRef;
use crate::scene::{MaterialAsset, MaterialComponent, RenderMode, Scene};

/// The range of the `[0, 1]` factors (metallic, roughness, alpha, alpha cutoff): the
/// setters clamp to it (a non-finite value reads as 0) and the editor's sliders span
/// it, so the limit lives here once, as `rigidbody::set_mass`'s does (#993).
pub const FACTOR_RANGE: RangeInclusive<f32> = 0.0..=1.0;

/// The scene's material library: library key -> shared asset. Matches
/// `Scene.materials`; aliased here so callers name one type.
pub type MaterialLibrary = BTreeMap<String, MaterialAsset>;

/// Define a named library asset, inserting (or overwriting) `asset` under `name`. The
/// single insert-by-name both the `Material.DefineAsset` binding and any future editor
/// "new material asset" entry use, so a standalone material is authored one way. The
/// asset's own fields are still validated by routing each value through the per-field
/// `set_*` ops (the API does this), so the clamps stay single-sourced.
pub fn define_asset(materials: &mut MaterialLibrary, name: &str, asset: MaterialAsset) {
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
/// inspector's card and a Play/Stop restore (`apply_scene_data`) both fold through it.
pub fn fold_pending(materials: &mut MaterialLibrary, key: &str, pending: MaterialAsset) {
    materials.entry(key.to_string()).or_insert(pending);
}

/// Resolve the library key for entity `id`'s first material, creating a default material
/// (under [`entity_material_key`]) and attaching the reference if it has none yet.
/// A library asset already under the key is kept, never overwritten: the rule every
/// attach path shares (#997).
/// Returns `None` only when the entity does not exist. This is the shared
/// resolve-or-create the API's setters use before applying an op; the editor card
/// already has a key in hand (it only edits an existing reference), so it does not
/// need it.
pub fn ensure_material_key(scene: &mut Scene, id: u32) -> Option<String> {
    if !scene.world.contains(id) {
        return None;
    }
    if !scene.world.has_material(id) {
        scene
            .world
            .set_material(id, Some(MaterialComponent::for_entity(id)));
    }
    let key = scene.world.material(id)?.first().to_string();
    scene.materials.entry(key.clone()).or_default();
    Some(key)
}

/// Set the base-color (albedo tint) factor.
pub fn set_base_color(materials: &mut MaterialLibrary, key: &str, rgb: [f32; 3]) {
    if let Some(m) = materials.get_mut(key) {
        m.base_color = rgb;
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

/// Set (or, with `None`, clear) the metallic map path.
pub fn set_metallic_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.metallic_map = path.map(MapRef::from);
    }
}

/// Set the roughness scalar factor, clamped to [`FACTOR_RANGE`].
pub fn set_roughness(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness = unit(value);
    }
}

/// Set (or, with `None`, clear) the roughness map path.
pub fn set_roughness_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness_map = path.map(MapRef::from);
    }
}

/// Set (or, with `None`, clear) the normal map path.
pub fn set_normal_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.normal_map = path.map(MapRef::from);
    }
}

/// Set the emissive factor.
pub fn set_emissive(materials: &mut MaterialLibrary, key: &str, rgb: [f32; 3]) {
    if let Some(m) = materials.get_mut(key) {
        m.emissive = rgb;
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

/// Set the authored surface shader by module name (#396); an empty name clears it
/// back to the standard forward shader. Whether the module exists is the renderer's
/// call (it falls back, logging once), so a shader baked later still binds.
pub fn set_shader(materials: &mut MaterialLibrary, key: &str, name: String) {
    if let Some(m) = materials.get_mut(key) {
        m.shader = (!name.is_empty()).then_some(name);
    }
}

/// Point material `key`'s extra shader texture `slot` (#400) at `path`; `None` or
/// `""` clears it back to white. An unknown slot is an error naming it and the
/// slots there are (#395). The path is not checked: a missing file samples white,
/// and a texture made later binds when it appears.
pub fn set_shader_texture(
    materials: &mut MaterialLibrary,
    key: &str,
    slot: &str,
    path: Option<String>,
) -> Result<(), String> {
    crate::shadergen::textures::slot(slot)?;
    if let Some(m) = materials.get_mut(key) {
        match path.filter(|p| !p.is_empty()) {
            Some(p) => m.shader_textures.insert(slot.to_owned(), p.into()),
            None => m.shader_textures.remove(slot),
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests;
