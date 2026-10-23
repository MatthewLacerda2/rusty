//! A re-import that drops bones warns about what was authored on them, and keeps
//! what it should of the bones it keeps (#1277): an override per bone entity, and
//! a bone moved out from under the rig left alone.

use glam::Vec3;

use super::fixture::{armature_owner, chain_skin, joint_to, reimport_to, warnings_of};
use crate::components::ColliderComponent;
use crate::scene::Scene;

/// Whether one of `warned` names bone `bone` and says `what`.
fn says(warned: &[String], bone: &str, what: &str) -> bool {
    let bone = format!("bone '{bone}'");
    warned.iter().any(|w| w.contains(&bone) && w.contains(what))
}

#[test]
fn a_dropped_bone_warns_about_its_override_physics_and_components() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let (spine, head) = (
        scene.find_bone(rig, "spine").unwrap(),
        scene.find_bone(rig, "head").unwrap(),
    );
    scene.world.transform_mut(head).unwrap().position = Vec3::X;
    let body = crate::scene::authoring::default_rigidbody();
    scene.world.set_rigidbody(spine, Some(body));
    joint_to(&mut scene, spine, None);
    let collider = ColliderComponent::from_mesh_bounds(true, -Vec3::ONE, Vec3::ONE);
    scene.world.set_collider(head, Some(collider));

    let warned = warnings_of(|| reimport_to(&mut scene, rig, chain_skin(&["hips"])));
    assert!(says(&warned, "head", "override was dropped"), "{warned:?}");
    assert!(says(&warned, "spine", "Rigidbody / Joint"), "{warned:?}");
    assert!(says(&warned, "head", "components it carried"), "{warned:?}");
    assert!(
        !says(&warned, "spine", "override"),
        "spine was at rest: {warned:?}"
    );
}

#[test]
fn two_bones_of_one_name_each_keep_their_own_override() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.world.mesh_mut(rig).unwrap().skin = Some(chain_skin(&["hips", "arm", "arm"]));
    scene.sync_skeleton(rig);
    let bones = scene.world.mesh(rig).unwrap().skeleton.ids();
    scene.world.transform_mut(bones[1]).unwrap().position = Vec3::X;
    scene.world.transform_mut(bones[2]).unwrap().position = Vec3::Z;

    reimport_to(&mut scene, rig, chain_skin(&["hips", "arm", "arm"]));

    assert_eq!(scene.world.mesh(rig).unwrap().skeleton.ids(), bones);
    let at = |b: u32| scene.world.transform(b).unwrap().position;
    assert_eq!((at(bones[1]), at(bones[2])), (Vec3::X, Vec3::Z));
}

#[test]
fn a_bone_moved_under_a_prop_is_left_there_by_a_reimport() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let head = scene.find_bone(rig, "head").unwrap();
    let prop = scene.add_entity("Prop".to_string());
    scene.set_parent(head, Some(prop)).unwrap();

    reimport_to(&mut scene, rig, chain_skin(&["hips", "spine", "head"]));

    assert_eq!(scene.world.parent_id(head), Some(prop), "left where it is");
    let new_head = scene.find_bone(rig, "head").unwrap();
    assert_ne!(new_head, head, "the rig got a head of its own");
    assert!(scene.is_bone(new_head) && !scene.is_bone(head));
}
