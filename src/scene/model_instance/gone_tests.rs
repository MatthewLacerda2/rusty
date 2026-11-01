//! A part whose node left the model (#1637, structural propagation is #1665) has
//! no local id, so `ListPrefabOverrides` names it by its node path.

use glam::Vec3;

use super::tests::{child, spawn_file, KIT};
use crate::scene::asset_instance::set_model_scale_factor;
use crate::scene::authoring::revert_instance_field;
use crate::scene::{list_instance_overrides, reimport_instance, revert_instance_overrides, Scene};

/// Spawn the kit as `file`, then rename its `Wall` node in the file, so the
/// placed `Wall` is gone from the model. Returns the scene, the root, the Wall
/// and the model's path.
fn spawn_then_rename(file: &str) -> (Scene, u32, u32, String) {
    let (scene, root) = spawn_file(file, KIT, Vec3::ZERO);
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let wall = child(&scene, child(&scene, root, "Kit"), "Wall");
    let renamed = KIT.replace(r#""name": "Wall""#, r#""name": "Panel""#);
    std::fs::write(&path, renamed).unwrap();
    (scene, root, wall, path)
}

fn max_corner(scene: &Scene, id: u32) -> Vec3 {
    let mesh = scene.world.mesh(id).unwrap();
    let positions = mesh.vertices.iter().map(|v| Vec3::from_array(v.position));
    positions.fold(Vec3::NEG_INFINITY, Vec3::max)
}

#[test]
fn a_gone_part_is_listed_by_its_node_path() {
    let (mut scene, root) = spawn_file("rusty_1637_gone.gltf", KIT, Vec3::ZERO);
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let kit = child(&scene, root, "Kit");
    let wall = child(&scene, kit, "Wall");
    scene.world.transform_mut(wall).unwrap().scale = Vec3::splat(2.0);
    let renamed = KIT.replace(r#""name": "Wall""#, r#""name": "Panel""#);
    std::fs::write(&path, renamed).unwrap();
    reimport_instance(&mut scene, root).unwrap();
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert!(
        listed
            .iter()
            .any(|o| o == "Kit/Wall (gone):/transform/scale/0"),
        "{listed:?}"
    );
    assert!(
        !listed.iter().any(|o| o.starts_with("4294967295")),
        "{listed:?}"
    );
}

#[test]
fn a_gone_part_takes_a_scale_factor_change() {
    let (mut scene, _, wall, path) = spawn_then_rename("rusty_1637_gone_scale.gltf");
    assert!(set_model_scale_factor(&mut scene, &path, 0.01).unwrap() > 0);
    let corner = max_corner(&scene, wall);
    assert!(
        corner.abs_diff_eq(Vec3::new(0.01, 0.01, 0.0), 1e-7),
        "a gone part is refit, not skipped: {corner}"
    );
}

#[test]
fn reverting_a_gone_part_says_so_and_keeps_its_override() {
    let (mut scene, root, wall, _) = spawn_then_rename("rusty_1637_gone_revert.gltf");
    scene.world.transform_mut(wall).unwrap().scale = Vec3::splat(2.0);
    reimport_instance(&mut scene, root).unwrap();
    let err = revert_instance_field(&mut scene, wall, "/transform/scale/0").unwrap_err();
    assert!(err.contains("Kit/Wall") && err.contains("gone"), "{err}");

    revert_instance_overrides(&mut scene, root).unwrap();
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert!(
        listed
            .iter()
            .any(|o| o == "Kit/Wall (gone):/transform/scale/0"),
        "{listed:?}"
    );
    assert_eq!(scene.world.transform(wall).unwrap().scale, Vec3::splat(2.0));
}

#[test]
fn another_spelling_of_the_model_path_finds_its_instances() {
    let (mut scene, root) = spawn_file("rusty_1637_spelling.gltf", KIT, Vec3::ZERO);
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let (dir, file) = path.rsplit_once('/').unwrap();
    let other = format!("{dir}/./{file}");
    let wall = child(&scene, child(&scene, root, "Kit"), "Wall");
    assert!(set_model_scale_factor(&mut scene, &other, 0.01).unwrap() > 0);
    let at = scene.world.transform(wall).unwrap().position;
    assert!(at.abs_diff_eq(Vec3::new(0.0, 0.0, 0.05), 1e-6), "{at}");
}
