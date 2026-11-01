//! What a joint holds hangs from the spawned bone (#1083, #453): a mesh on the
//! joint itself and a non-joint child (a gun under the hand), so both follow it.

use glam::Vec3;

use super::tests::child;
use crate::asset::fixtures_anim::{animated_buffer, ANIMATED_GLTF};
use crate::scene::Scene;

/// The animated rig with a `Prop` mesh on `Joint1` and a `Gun` under it 1 m
/// along Z, spawned whole.
fn rig() -> (Scene, u32) {
    rig_in("rusty_1083_rig")
}

/// [`rig`], its file in the test folder's `dir`.
fn rig_in(dir: &str) -> (Scene, u32) {
    let path = write_rig(dir, false);
    let asset = crate::asset::import_and_sync_sidecar(&path).unwrap();
    let mut scene = Scene::new();
    let path = path.to_string_lossy().into_owned();
    let root = super::instantiate_model(&mut scene, &path, &asset, Vec3::ZERO);
    (scene, root)
}

/// Write the rig's file into `dir`, with a `Scope` node under `Joint1` too when
/// `scope`.
fn write_rig(dir: &str, scope: bool) -> std::path::PathBuf {
    let mut gltf: serde_json::Value = serde_json::from_str(ANIMATED_GLTF).unwrap();
    let prop = serde_json::json!({
        "name": "Prop", "primitives": [ { "attributes": { "POSITION": 0 } } ]
    });
    gltf["meshes"].as_array_mut().unwrap().push(prop);
    gltf["nodes"][2]["mesh"] = 1.into();
    gltf["nodes"][2]["children"] = serde_json::json!([3]);
    let gun = serde_json::json!({ "name": "Gun", "mesh": 1, "translation": [0.0, 0.0, 1.0] });
    gltf["nodes"].as_array_mut().unwrap().push(gun);
    if scope {
        gltf["nodes"][2]["children"] = serde_json::json!([3, 4]);
        let scope = serde_json::json!({ "name": "Scope", "translation": [0.0, 0.0, 2.0] });
        gltf["nodes"].as_array_mut().unwrap().push(scope);
    }
    let dir = crate::test_temp::dir().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("anim.bin"), animated_buffer()).unwrap();
    let path = dir.join("model.gltf");
    std::fs::write(&path, gltf.to_string()).unwrap();
    path
}

#[test]
fn a_mesh_on_a_joint_is_spawned_at_its_bone() {
    let (scene, root) = rig();
    let body = child(&scene, root, "MeshNode");
    let bone = scene
        .find_bone(body, "Joint1")
        .expect("the skeleton is spawned");
    let prop = child(&scene, bone, "Joint1");
    assert!(scene.world.mesh(prop).is_some(), "the joint's mesh is kept");
    assert_eq!(scene.world.transform(prop).unwrap().position, Vec3::ZERO);
}

#[test]
fn a_child_of_a_joint_hangs_from_the_bone_and_follows_it() {
    let (mut scene, root) = rig();
    let body = child(&scene, root, "MeshNode");
    let gun = child(&scene, scene.find_bone(body, "Joint1").unwrap(), "Gun");
    let at = scene.world.world_position(gun);
    assert!(
        at.abs_diff_eq(Vec3::new(0.0, 1.0, 1.0), 1e-5),
        "at rest: {at}"
    );
    let hips = scene.find_bone(body, "Joint0").unwrap();
    scene.world.transform_mut(hips).unwrap().position = Vec3::X * 2.0;
    let moved = scene.world.world_position(gun);
    assert!(
        moved.abs_diff_eq(Vec3::new(2.0, 1.0, 1.0), 1e-5),
        "posed: {moved}"
    );
}

#[test]
fn a_node_added_under_a_joint_hangs_from_its_bone() {
    let (mut scene, root) = rig_in("rusty_1665_rig");
    write_rig("rusty_1665_rig", true);
    crate::scene::reimport_instance(&mut scene, root).unwrap();
    let body = child(&scene, root, "MeshNode");
    let bone = scene.find_bone(body, "Joint1").unwrap();
    let scope = child(&scene, bone, "Scope");
    let at = scene.world.world_position(scope);
    assert!(at.abs_diff_eq(Vec3::new(0.0, 1.0, 2.0), 1e-5), "{at}");
    assert!(scene
        .world
        .prefab_link(scope)
        .is_some_and(|l| l.local_id != 0));
}

#[test]
fn a_gone_skinned_part_takes_its_bones_hitboxes() {
    use crate::components::ColliderComponent;
    let (mut scene, root) = rig_in("rusty_1665_hitbox");
    let body = child(&scene, root, "MeshNode");
    let bone = scene.find_bone(body, "Joint1").unwrap();
    let hitbox = scene.add_entity(crate::scene::skeleton::HITBOX_NAME.to_string());
    scene.set_parent(hitbox, Some(bone)).unwrap();
    let collider = ColliderComponent::from_mesh_bounds(true, -Vec3::ONE, Vec3::ONE);
    scene.world.set_collider(hitbox, Some(collider));
    let path = write_rig("rusty_1665_hitbox", false);
    let gltf = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, gltf.replace("\"MeshNode\"", "\"Body\"")).unwrap();
    crate::scene::reimport_instance(&mut scene, root).unwrap();
    assert!(!scene.world.contains(body));
    assert!(
        !scene.world.contains(hitbox),
        "destroyed with its bone, not re-homed"
    );
}
