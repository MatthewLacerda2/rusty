//! A material the bake reads that its scene file lacks (#1958): the bake names
//! it, a fresh load of the file reads the bake as stale, and a save that writes
//! the material makes the next load match (the operator's second editor load).

use std::collections::BTreeMap;
use std::sync::Arc;

use super::test_level::corridors;
use super::{bake_occlusion, fingerprint, save_sidecar, unsaved_materials};
use crate::authoring::material::define_asset;
use crate::{parse_scene_text, MaterialAsset, MaterialComponent, Scene};
use rusty_components::RenderFace;

/// The library of the scene file at `path`.
fn file_materials(path: &str) -> BTreeMap<String, MaterialAsset> {
    let text = std::fs::read_to_string(path).unwrap();
    parse_scene_text(&text, path).unwrap().materials
}

/// How many of the PVS's objects a fresh load of `path` reads as changed.
fn stale_on_load(path: &str) -> usize {
    let mut fresh = Scene::new();
    fresh.load_from_file(path).unwrap();
    let pvs = fresh.occlusion.as_deref().expect("the bake loaded");
    let objects = pvs.objects.iter();
    objects
        .filter(|o| fingerprint(&fresh, o.entity) != Some(o.fingerprint))
        .count()
}

#[test]
fn a_material_the_file_lacks_is_named_and_a_save_that_writes_it_makes_the_load_match() {
    let dir = rusty_test_support::temp::dir().join("occlusion_unsaved_material");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("level.scene").to_string_lossy().into_owned();
    let mut level = corridors();
    for id in level.world.ids_with_mesh() {
        level
            .world
            .set_material(id, Some(MaterialComponent::new("glass")));
    }
    level.materials.remove("glass");
    level.save_to_file(&path).unwrap();

    let mut baking = Scene::new();
    baking.load_from_file(&path).unwrap();
    let default_like = MaterialAsset::default();
    define_asset(&mut baking.materials, "glass", default_like);
    assert!(
        unsaved_materials(&baking, &file_materials(&path)).is_empty(),
        "a default-like material reads as the missing key does"
    );
    let two_sided = MaterialAsset {
        render_face: RenderFace::Both,
        ..MaterialAsset::default()
    };
    define_asset(&mut baking.materials, "glass", two_sided);
    let named = unsaved_materials(&baking, &file_materials(&path));
    assert_eq!(named, ["glass"]);
    let data = bake_occlusion(&baking, 2).0.expect("occluders");
    *baking.occlusion = Some(Arc::new(data));
    save_sidecar(&baking, &path).unwrap();
    assert!(stale_on_load(&path) > 0, "the file draws glass one-sided");

    baking.save_to_file(&path).unwrap();
    assert!(unsaved_materials(&baking, &file_materials(&path)).is_empty());
    assert_eq!(stale_on_load(&path), 0, "saved with its materials");
}
