//! A whole-model spawn is a linked instance of the model (#1637, Unity's model
//! prefab): a Scale Factor change or a re-written file reaches its parts, in the
//! open scene and in a saved one, and a part moved by hand keeps its override.

use glam::Vec3;

use super::tests::{child, spawn_file, KIT};
use crate::scene::asset_instance::{instantiate_asset, set_model_scale_factor};
use crate::scene::prefab::apply::apply_instance_to_source;
use crate::scene::serialize::{apply_scene_data, to_scene_data};
use crate::scene::{list_instance_overrides, reimport_instance, Scene};

fn path_of(scene: &Scene, root: u32) -> String {
    scene.world.prefab_link(root).unwrap().source.clone()
}

#[test]
fn every_entity_links_to_the_model_by_node_path() {
    let (scene, root) = spawn_file("rusty_1637_link.gltf", KIT, Vec3::ZERO);
    let link = scene.world.prefab_link(root).unwrap().clone();
    assert!(link.source.ends_with("rusty_1637_link.gltf"));
    assert_eq!((link.local_id, link.node.as_deref()), (0, Some("")));
    let kit = child(&scene, root, "Kit");
    let wall = child(&scene, kit, "Wall");
    let node = |id: u32| scene.world.prefab_link(id).unwrap().node.clone();
    assert_eq!(node(wall).as_deref(), Some("Kit/Wall"));
    let fresh = list_instance_overrides(&scene, root).unwrap();
    assert!(
        fresh.is_empty(),
        "a fresh spawn has no overrides: {fresh:?}"
    );
    let key = &scene.world.material(wall).unwrap().materials[0];
    assert_eq!(*key, format!("{}::#default", link.source), "one per model");
}

#[test]
fn a_lone_sub_object_spawn_is_not_linked() {
    let (mut scene, root) = spawn_file("rusty_1637_sub.gltf", KIT, Vec3::ZERO);
    let reference = format!("{}::WallMesh", path_of(&scene, root));
    let id = instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap();
    assert!(scene.world.prefab_link(id).is_none());
}

#[test]
fn a_scale_factor_moves_the_parts_and_keeps_a_hand_edit() {
    let at = Vec3::new(3.0, 0.0, 0.0);
    let (mut scene, root) = spawn_file("rusty_1637_scale.gltf", KIT, at);
    let path = path_of(&scene, root);
    let kit = child(&scene, root, "Kit");
    let wall = child(&scene, kit, "Wall");
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(0.0, 7.0, 0.0);

    assert!(set_model_scale_factor(&mut scene, &path, 0.01).unwrap() > 0);
    assert!(scene
        .world
        .transform(kit)
        .unwrap()
        .position
        .abs_diff_eq(Vec3::X * 0.1, 1e-6));
    let moved = scene.world.transform(wall).unwrap().position;
    assert_eq!(
        moved,
        Vec3::new(0.0, 7.0, 0.0),
        "the hand edit is an override"
    );
    assert_eq!(
        scene.world.transform(root).unwrap().position,
        at,
        "the root stays put"
    );
    let group = child(&scene, kit, "Crate");
    let crate_y = scene.world.transform(group).unwrap().position.y;
    assert!((crate_y - 0.02).abs() < 1e-6, "{crate_y}");
    assert_eq!(scene.undo(), None, "a re-stamp is no edit to undo");
}

#[test]
fn a_saved_scene_takes_the_new_scale_on_load() {
    let (scene, root) = spawn_file("rusty_1637_saved.gltf", KIT, Vec3::ZERO);
    let path = path_of(&scene, root);
    let data = to_scene_data(&scene);
    // Changed while the scene is closed: nothing open to re-stamp.
    set_model_scale_factor(&mut Scene::new(), &path, 0.5).unwrap();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, data);
    let kit = child(&loaded, root, "Kit");
    assert_eq!(loaded.world.transform(kit).unwrap().position, Vec3::X * 5.0);
}

#[test]
fn an_added_node_keeps_each_part_on_its_own_node() {
    let (mut scene, root) = spawn_file("rusty_1637_added.gltf", KIT, Vec3::ZERO);
    let path = path_of(&scene, root);
    let kit = child(&scene, root, "Kit");
    let wall = child(&scene, kit, "Wall");
    scene.world.transform_mut(wall).unwrap().scale = Vec3::splat(2.0);
    // A node inserted ahead of Wall shifts every later local id.
    let edited = KIT
        .replace(
            r#""children": [ 1, 2, 3 ]"#,
            r#""children": [ 4, 1, 2, 3 ]"#,
        )
        .replace(
            r#"{ "name": "Crate_LOD1", "mesh": 2, "translation": [0.0, 3.0, 0.0] }"#,
            r#"{ "name": "Crate_LOD1", "mesh": 2, "translation": [0.0, 3.0, 0.0] },
    { "name": "Door", "translation": [1.0, 0.0, 0.0] }"#,
        )
        .replace(
            r#""translation": [0.0, 0.0, 5.0]"#,
            r#""translation": [0.0, 0.0, 6.0]"#,
        );
    std::fs::write(&path, edited).unwrap();
    reimport_instance(&mut scene, root).unwrap();

    assert_eq!(*scene.world.name(wall).unwrap(), "Wall");
    let t = scene.world.transform(wall).unwrap().clone();
    assert_eq!(
        t.position,
        Vec3::Z * 6.0,
        "the moved node is taken from the file"
    );
    assert_eq!(t.scale, Vec3::splat(2.0), "the hand edit stays on Wall");
    let door = child(&scene, kit, "Door");
    let link = scene.world.prefab_link(door).unwrap();
    assert_eq!(
        link.node.as_deref(),
        Some("Kit/Door"),
        "the new node is spawned"
    );
    assert_eq!(scene.world.transform(door).unwrap().position, Vec3::X);
}

#[test]
fn a_model_refuses_an_apply_to_source() {
    let (mut scene, root) = spawn_file("rusty_1637_apply.gltf", KIT, Vec3::ZERO);
    let kit = child(&scene, root, "Kit");
    scene.world.transform_mut(kit).unwrap().scale = Vec3::splat(2.0);
    let err = apply_instance_to_source(&mut scene, root).unwrap_err();
    assert!(err.contains("is a model"), "{err}");
}
