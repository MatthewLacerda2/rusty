//! src/scene/authoring/material.rs — Shared material-authoring ops.
//!
//! The ONE place the engine knows how to mutate a library `MaterialAsset` field by
//! field (the PBR factors, the glTF-PBR map paths, and the transparency story —
//! render mode + alpha + cutoff, #242), including the validation that belongs to
//! each write (the `[0, 1]` clamp on `alpha`/`alpha_cutoff`).
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
//! Allowed deps: components (the `MaterialAsset`/`MaterialComponent`/`RenderMode`
//! data), scene (the library type + `Scene` for `ensure_material_key`). Pure: no
//! wall-clock, no RNG.

use std::collections::BTreeMap;

use crate::scene::{MaterialAsset, MaterialComponent, RenderMode, Scene};

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

/// Resolve the library key for entity `id`'s material, creating a default material
/// (under `entity_{id}_material`) and attaching the reference if it has none yet.
/// Returns `None` only when the entity does not exist. This is the shared
/// resolve-or-create the API's setters use before applying an op; the editor card
/// already has a key in hand (it only edits an existing reference), so it does not
/// need it.
pub fn ensure_material_key(scene: &mut Scene, id: u32) -> Option<String> {
    if !scene.world.contains(id) {
        return None;
    }
    if !scene.world.has_material(id) {
        scene.world.set_material(
            id,
            Some(MaterialComponent {
                material: format!("entity_{id}_material"),
            }),
        );
    }
    let key = scene.world.material(id).unwrap().material.clone();
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
        m.base_color_map = (!path.is_empty()).then_some(path);
    }
}

/// Set the metallic scalar factor.
pub fn set_metallic(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.metallic = value;
    }
}

/// Set (or, with `None`, clear) the metallic map path.
pub fn set_metallic_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.metallic_map = path;
    }
}

/// Set the roughness scalar factor.
pub fn set_roughness(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness = value;
    }
}

/// Set (or, with `None`, clear) the roughness map path.
pub fn set_roughness_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.roughness_map = path;
    }
}

/// Set (or, with `None`, clear) the normal map path.
pub fn set_normal_map(materials: &mut MaterialLibrary, key: &str, path: Option<String>) {
    if let Some(m) = materials.get_mut(key) {
        m.normal_map = path;
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
        m.emissive_map = path;
    }
}

/// Set the compositing/render mode (typed — string parsing stays in the API adapter).
pub fn set_render_mode(materials: &mut MaterialLibrary, key: &str, mode: RenderMode) {
    if let Some(m) = materials.get_mut(key) {
        m.render_mode = mode;
    }
}

/// Set the base-color alpha, clamped to `[0, 1]`. The clamp validation lives HERE,
/// once — both the editor slider and the `Material.SetAlpha` binding inherit it.
pub fn set_alpha(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.alpha = value.clamp(0.0, 1.0);
    }
}

/// Set the alpha-test cutoff, clamped to `[0, 1]` (the single source of that
/// validation, shared by the editor slider and `Material.SetAlphaCutoff`).
pub fn set_alpha_cutoff(materials: &mut MaterialLibrary, key: &str, value: f32) {
    if let Some(m) = materials.get_mut(key) {
        m.alpha_cutoff = value.clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A library with one default material under `"k"`.
    fn one() -> MaterialLibrary {
        let mut materials = MaterialLibrary::new();
        materials.insert("k".to_string(), MaterialAsset::default());
        materials
    }

    #[test]
    fn scalar_and_color_ops_write_through() {
        let mut materials = one();
        set_metallic(&mut materials, "k", 0.7);
        set_roughness(&mut materials, "k", 0.2);
        set_base_color(&mut materials, "k", [0.1, 0.2, 0.3]);
        set_emissive(&mut materials, "k", [1.0, 0.0, 0.0]);
        let m = &materials["k"];
        assert_eq!(m.metallic, 0.7);
        assert_eq!(m.roughness, 0.2);
        assert_eq!(m.base_color, [0.1, 0.2, 0.3]);
        assert_eq!(m.emissive, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn map_ops_set_and_clear() {
        let mut materials = one();
        set_base_color_map(&mut materials, "k", "albedo.png".to_string());
        set_base_color_map(&mut materials, "k", String::new()); // empty clears
        set_metallic_map(&mut materials, "k", Some("m.png".to_string()));
        set_normal_map(&mut materials, "k", None);
        let m = &materials["k"];
        assert_eq!(m.base_color_map, None);
        assert_eq!(m.metallic_map.as_deref(), Some("m.png"));
        assert_eq!(m.normal_map, None);
    }

    #[test]
    fn alpha_and_cutoff_clamp_to_unit_range() {
        let mut materials = one();
        set_alpha(&mut materials, "k", 2.0);
        set_alpha_cutoff(&mut materials, "k", -1.0);
        let m = &materials["k"];
        assert_eq!(m.alpha, 1.0, "alpha clamps to the upper bound");
        assert_eq!(m.alpha_cutoff, 0.0, "cutoff clamps to the lower bound");
    }

    #[test]
    fn render_mode_op_writes_typed_mode() {
        let mut materials = one();
        set_render_mode(&mut materials, "k", RenderMode::Transparent);
        assert_eq!(materials["k"].render_mode, RenderMode::Transparent);
    }

    #[test]
    fn op_on_missing_key_is_a_no_op() {
        let mut materials = one();
        set_metallic(&mut materials, "absent", 0.9);
        assert_eq!(materials["k"].metallic, MaterialAsset::default().metallic);
    }

    #[test]
    fn define_asset_inserts_and_overwrites_by_name() {
        let mut materials = MaterialLibrary::new();
        define_asset(
            &mut materials,
            "brick",
            MaterialAsset {
                metallic: 0.3,
                ..MaterialAsset::default()
            },
        );
        assert_eq!(materials["brick"].metallic, 0.3);
        // A second define under the same name overwrites (insert semantics).
        define_asset(
            &mut materials,
            "brick",
            MaterialAsset {
                metallic: 0.9,
                ..MaterialAsset::default()
            },
        );
        assert_eq!(materials["brick"].metallic, 0.9);
        assert_eq!(materials.len(), 1);
    }

    #[test]
    fn ensure_named_creates_default_but_never_overwrites() {
        let mut materials = MaterialLibrary::new();
        ensure_named(&mut materials, "k");
        // Mutate via a per-field op, then ensure again: the existing asset is kept.
        set_metallic(&mut materials, "k", 0.6);
        ensure_named(&mut materials, "k");
        assert_eq!(materials["k"].metallic, 0.6, "ensure does not reset");
        assert_eq!(materials.len(), 1);
    }

    #[test]
    fn ensure_material_key_resolves_then_creates() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Box".to_string());
        let key = ensure_material_key(&mut scene, id).expect("entity exists");
        assert_eq!(key, format!("entity_{id}_material"));
        assert!(scene.materials.contains_key(&key), "library entry created");
        assert!(scene.world.has_material(id), "ref attached");
        // Idempotent: a second resolve returns the same key, no duplicate entry.
        assert_eq!(
            ensure_material_key(&mut scene, id).as_deref(),
            Some(key.as_str())
        );
        assert_eq!(scene.materials.len(), 1);
    }
}
