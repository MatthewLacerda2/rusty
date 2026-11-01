//! A part whose node the model lost goes the way the moment allows (#1665): at
//! once while a document is applied, a Play-mode `Scene.Load` included, and
//! through the destroy queue for a Play-mode reimport, so its scripts are torn
//! down. A part of the instance that survives under it moves out with what hangs
//! under it, its overrides kept.

use std::collections::BTreeSet;

use glam::Vec3;

use super::gone_tests::spawn_then_rename;
use super::tests::{child, spawn_file, KIT};
use crate::scene::serialize::to_scene_data;
use crate::scene::{list_instance_overrides, reimport_instance};

#[test]
fn a_play_mode_load_removes_a_gone_part() {
    let (scene, _, _, wall, _) = spawn_then_rename("rusty_1665_play_load.gltf");
    let data = to_scene_data(&scene);
    let mut playing = crate::scene::Scene::new();
    playing.set_playing(true);
    playing
        .swap_in(data, "rusty_1665_no_sidecar.scene", &BTreeSet::new())
        .unwrap();
    assert!(!playing.world.contains(wall), "not left behind by the swap");
}

#[test]
fn a_play_mode_reimport_queues_a_gone_part() {
    let (mut scene, root, _, wall, _) = spawn_then_rename("rusty_1665_play_reimport.gltf");
    scene.set_playing(true);
    reimport_instance(&mut scene, root).unwrap();
    assert!(scene.is_doomed(wall), "torn down at the destroy phase");
}

#[test]
fn a_part_under_a_gone_part_moves_out_with_its_overrides() {
    let mut gltf: serde_json::Value = serde_json::from_str(KIT).unwrap();
    let post = serde_json::json!({ "name": "Post", "mesh": 0 });
    gltf["nodes"].as_array_mut().unwrap().push(post);
    gltf["nodes"][0]["children"] = serde_json::json!([1, 2, 3, 4]);
    let file = "rusty_1665_survivor.gltf";
    let (mut scene, root) = spawn_file(file, &gltf.to_string(), Vec3::ZERO);
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let kit = child(&scene, root, "Kit");
    let (wall, post) = (child(&scene, kit, "Wall"), child(&scene, kit, "Post"));
    scene.set_parent(post, Some(wall)).unwrap();
    let lamp = crate::scene::authoring::create_entity(&mut scene, "Lamp", None);
    scene.set_parent(lamp, Some(post)).unwrap();
    scene.world.transform_mut(post).unwrap().scale = Vec3::splat(3.0);
    gltf["nodes"][1]["name"] = "Panel".into();
    std::fs::write(&path, gltf.to_string()).unwrap();
    reimport_instance(&mut scene, root).unwrap();
    assert!(!scene.world.contains(wall));
    assert_eq!(
        scene.world.parent_id(post),
        Some(kit),
        "re-homed, not re-stamped"
    );
    assert_eq!(
        scene.world.parent_id(lamp),
        Some(post),
        "with what hangs under it"
    );
    let scale = scene.world.transform(post).unwrap().scale;
    assert!(scale.abs_diff_eq(Vec3::splat(3.0), 1e-5), "{scale}");
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert!(
        listed.iter().any(|k| k.contains("/transform/scale")),
        "{listed:?}"
    );
}
