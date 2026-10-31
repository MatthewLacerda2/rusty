//! A part whose node left the model (#1637, structural propagation is #1665) has
//! no local id, so `ListPrefabOverrides` names it by its node path.

use glam::Vec3;

use super::tests::{child, spawn_file, KIT};
use crate::scene::{list_instance_overrides, reimport_instance};

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
