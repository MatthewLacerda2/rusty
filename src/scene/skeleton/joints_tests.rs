//! A joint from outside a skeleton to a bone survives a save, a load and a
//! re-import by the bone's name (#1264).

use super::fixture::{armature_owner, hero};
use crate::components::JointComponent;
use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::scene::Scene;

/// A `Rope` entity jointed to `owner`'s bone `bone`.
fn rope_to(scene: &mut Scene, owner: u32, bone: &str) -> u32 {
    let rope = scene.add_entity("Rope".to_string());
    let joint = JointComponent {
        connected_body: scene.find_bone(owner, bone),
        ..JointComponent::default()
    };
    scene.world.set_joint(rope, Some(joint));
    rope
}

fn connected(scene: &Scene, id: u32) -> Option<u32> {
    scene.world.joint(id).unwrap().connected_body
}

fn reload(scene: &Scene) -> Scene {
    let json = serde_json::to_string(&to_scene_data(scene)).unwrap();
    let data: SceneData = serde_json::from_str(&json).unwrap();
    let mut out = Scene::new();
    apply_scene_data(&mut out, data);
    out
}

#[test]
fn a_joint_to_a_bone_is_saved_by_name_and_rebound_on_load() {
    let mut scene = Scene::new();
    let hero = hero(&mut scene, "joint_to_bone");
    let rope = rope_to(&mut scene, hero, "Joint1");

    let data = to_scene_data(&scene);
    let saved = data.entities.iter().find(|e| e.id == rope).unwrap();
    let joint = saved.joint.as_ref().unwrap();
    assert_eq!(joint.connected_body, Some(hero), "the skinned entity");
    assert_eq!(
        joint.connected_bone,
        ["Joint1", "Joint0"],
        "the bone, then its ancestors"
    );

    let loaded = reload(&scene);
    let bone = loaded.find_bone(hero, "Joint1").unwrap();
    assert_eq!(connected(&loaded, rope), Some(bone));
    let live = loaded.world.joint(rope).unwrap();
    assert!(live.connected_bone.is_empty(), "document-only");
}

#[test]
fn a_reimport_rebinds_a_joint_to_the_new_bone() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let rope = rope_to(&mut scene, rig, "spine");
    let old = connected(&scene, rope).unwrap();

    let hung = scene.take_skeleton(rig);
    assert_eq!(connected(&scene, rope), Some(rig), "parked on the rig");
    scene.restore_skeleton(rig, hung);
    let spine = scene.find_bone(rig, "spine").unwrap();
    assert_ne!(spine, old, "the bone was rebuilt");
    assert_eq!(connected(&scene, rope), Some(spine));
    assert!(scene.world.joint(rope).unwrap().connected_bone.is_empty());
}

#[test]
fn a_joint_to_a_bone_the_rig_lost_goes_to_its_nearest_ancestor() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let rope = rope_to(&mut scene, rig, "head");
    let hung = scene.take_skeleton(rig);
    // The new rig drops `head`: `hips → spine` is left.
    let mut skin = scene.world.mesh(rig).unwrap().skin.clone().unwrap();
    skin.names.truncate(2);
    skin.local_bind.truncate(2);
    skin.parents.truncate(2);
    skin.bind_global.truncate(2);
    skin.inverse_bind.truncate(2);
    skin.joint_nodes.truncate(2);
    scene.world.mesh_mut(rig).unwrap().skin = Some(skin);

    scene.restore_skeleton(rig, hung);
    assert_eq!(connected(&scene, rope), scene.find_bone(rig, "spine"));
}

#[test]
fn a_joint_between_two_props_is_left_alone() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let post = scene.add_entity("Post".to_string());
    let rope = rope_to(&mut scene, rig, "spine");
    scene.world.joint_mut(rope).unwrap().connected_body = Some(post);
    let hung = scene.take_skeleton(rig);
    scene.restore_skeleton(rig, hung);
    assert_eq!(connected(&scene, rope), Some(post));
}
