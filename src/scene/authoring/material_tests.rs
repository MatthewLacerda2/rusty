//! Tests for the shared material-authoring ops.

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
fn shader_op_sets_and_empty_clears() {
    let mut materials = one();
    set_shader(&mut materials, "k", "enemy_toon".to_string());
    assert_eq!(materials["k"].shader.as_deref(), Some("enemy_toon"));
    set_shader(&mut materials, "k", String::new());
    assert_eq!(materials["k"].shader, None);
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
