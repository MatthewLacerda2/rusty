//! A collision mesh (#1960) that stops loading falls back to its entity's own mesh,
//! on a load and on a live re-import, and a re-import follows the model otherwise.

use std::collections::BTreeSet;

use super::tests::{chair, spawn, triangles};
use crate::asset_instance::{instantiate_asset, set_model_scale_factor};
use crate::rehydrate::{reimport_model_except, Rig};
use crate::serialize::{apply_scene_data, to_scene_data};
use crate::Scene;

#[test]
fn a_scale_factor_change_reaches_the_collision_mesh() {
    let path = chair("rusty_1960_chair_scale.obj", 4, Some("Chair_col"));
    let (mut scene, _) = spawn(&path);
    set_model_scale_factor(&mut scene, &path, 2.0).unwrap();
    let chair = scene.world.ids_with_collider().into_iter().next().unwrap();
    assert_eq!(triangles(&scene, chair), 2);
    let top = scene.world.collider(chair).unwrap().aabb_max.y;
    assert!((top - 1.0).abs() < 1e-5, "the sibling scaled too: {top}");
}

#[test]
fn a_collision_mesh_that_no_longer_loads_falls_back_on_load() {
    // A lone part: a whole model is rebuilt from its sidecar on load instead.
    let path = chair("rusty_1960_chair_gone_load.obj", 4, Some("Chair_col"));
    let mut scene = Scene::new();
    let part = format!("{path}::Chair");
    let id = instantiate_asset(&mut scene, &part, None, glam::Vec3::ZERO).unwrap();
    assert_eq!(triangles(&scene, id), 2);
    let text = serde_json::to_string(&to_scene_data(&scene)).unwrap();
    // Renamed in the .blend since the save.
    let text = text.replace("::Chair_col", "::Chair_renamed");
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, serde_json::from_str(&text).unwrap());
    assert_eq!(
        triangles(&loaded, id),
        4 * 4 * 2,
        "the own mesh, not nothing"
    );
    let top = loaded.world.collider(id).unwrap().aabb_max.y;
    assert!((top - 0.1).abs() < 1e-5, "the box is the own mesh's: {top}");
    let warned = format!("{path}::Chair_renamed");
    assert!(crate::rehydrate::warned_of(&warned));
}

#[test]
fn a_re_import_without_the_collision_mesh_falls_back() {
    let path = chair("rusty_1960_chair_gone_live.obj", 4, Some("Chair_col"));
    let (mut scene, kids) = spawn(&path);
    let id = kids[0].1;
    let obj = std::fs::read_to_string(&path).unwrap();
    let cut = obj.find("o Chair_col").unwrap();
    std::fs::write(&path, &obj[..cut]).unwrap();
    set_model_scale_factor(&mut scene, &path, 1.5).unwrap();
    let c = scene.world.collider(id).unwrap();
    assert!(
        c.collision_mesh.as_ref().unwrap().indices.is_empty(),
        "no old triangles"
    );
    assert_eq!(triangles(&scene, id), 4 * 4 * 2, "the own mesh");
    assert!(
        (c.aabb_max.y - 0.15).abs() < 1e-5,
        "refit to it: {}",
        c.aabb_max.y
    );
}

#[test]
fn a_mesh_naming_its_own_sub_object_is_its_own() {
    let path = chair("rusty_1960_chair_self.obj", 3, Some("Chair"));
    let (scene, kids) = spawn(&path);
    let names: Vec<&str> = kids.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["Chair", "Chair_col"], "nothing is hidden");
    let id = kids[0].1;
    assert_eq!(scene.world.collider(id).unwrap().collision_mesh, None);
    assert_eq!(triangles(&scene, id), 3 * 3 * 2);
}

#[test]
fn a_skipped_entity_keeps_its_collision_mesh() {
    let path = chair("rusty_1960_chair_skip.obj", 4, Some("Chair_col"));
    let (mut scene, kids) = spawn(&path);
    let id = kids[0].1;
    let mut settings = rusty_asset::sidecar::load(std::path::Path::new(&path)).unwrap();
    settings.scale = 2.0;
    rusty_asset::sidecar::save(std::path::Path::new(&path), &settings).unwrap();
    reimport_model_except(&mut scene, &path, Rig::Rebuild, &BTreeSet::from([id]));
    let top = scene.world.collider(id).unwrap().aabb_max.y;
    assert_eq!(top, 0.5, "left to whoever skipped it");
}
