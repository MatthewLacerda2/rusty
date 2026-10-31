//! What the bone systems cost a scene without a ragdoll (#1617): `pose_bones`
//! never reads a skeleton, and `follow_bones` still moves a hitbox with its bone
//! while leaving the map's colliders alone.

use glam::{Quat, Vec3};

use crate::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use crate::physics::PhysicsWorld;
use crate::scene::skeleton::fixture::armature_owner;
use crate::scene::skeleton::probe;
use crate::scene::Scene;

fn collider() -> ColliderComponent {
    let shape = ColliderShape::Box {
        size: Vec3::splat(0.2),
    };
    ColliderComponent {
        shape,
        ..crate::scene::authoring::default_collider()
    }
}

/// Two skinned rigs, a hitbox under each rig's `spine`, a static wall and a
/// dynamic crate: everything a level has but a ragdoll. Returns the hitboxes.
fn level(scene: &mut Scene) -> Vec<u32> {
    let mut hitboxes = Vec::new();
    for _ in 0..2 {
        let rig = armature_owner(scene);
        scene.sync_skeletons();
        let spine = scene.find_bone(rig, "spine").unwrap();
        let hb = scene.add_entity("Hitbox".to_string());
        scene.set_parent(hb, Some(spine)).unwrap();
        scene.world.set_collider(hb, Some(collider()));
        hitboxes.push(hb);
    }
    let wall = scene.add_entity("Wall".to_string());
    scene.world.set_collider(wall, Some(collider()));
    let crate_id = scene.add_entity("Crate".to_string());
    scene.world.transform_mut(crate_id).unwrap().position = Vec3::Y * 5.0;
    scene.world.set_collider(crate_id, Some(collider()));
    let rb = RigidBodyComponent {
        is_kinematic: false,
        ..crate::scene::authoring::defaults::default_rigidbody()
    };
    scene.world.set_rigidbody(crate_id, Some(rb));
    hitboxes
}

#[test]
fn without_a_ragdoll_pose_bones_reads_no_skeleton() {
    let mut scene = Scene::new();
    level(&mut scene);
    let physics = PhysicsWorld::from_scene(&scene);
    let before = probe::reads();
    physics.pose_bones(&mut scene);
    assert_eq!(probe::reads(), before, "no skeleton was read");
    // The probe counts: the bone set the old path built reads both skeletons.
    scene.bone_ids();
    assert_eq!(probe::reads(), before + 2);
}

#[test]
fn follow_bones_moves_a_hitbox_with_its_bone() {
    let mut scene = Scene::new();
    let hitboxes = level(&mut scene);
    let mut physics = PhysicsWorld::from_scene(&scene);
    let spine = scene.world.parent_id(hitboxes[0]).unwrap();
    let hips = scene.world.parent_id(spine).unwrap();
    scene.world.transform_mut(hips).unwrap().rotation = Quat::from_rotation_z(1.0);
    let handle = physics.id_to_collider[&hitboxes[0]];
    let stale = Vec3::from(physics.colliders[handle].position().translation);
    let want = scene.world.world_pose(hitboxes[0]).unwrap().pos;
    assert!((stale - want).length() > 0.1, "the swing moved the hitbox");
    physics.follow_bones(&scene);
    let at = physics.colliders[handle].position().translation;
    assert!(
        (Vec3::from(at) - want).length() < 1e-5,
        "{at:?} vs {want:?}"
    );
}

#[test]
fn bone_subtrees_hold_bones_and_what_hangs_from_them_only() {
    let mut scene = Scene::new();
    let hitboxes = level(&mut scene);
    let rig = scene.world.ids_with_mesh()[0];
    let beside = scene.add_entity("Beside".to_string());
    scene.set_parent(beside, Some(rig)).unwrap();
    let hung = scene.bone_subtrees();
    let mut bones = scene.bone_ids();
    bones.extend(&hitboxes);
    assert_eq!(hung, bones, "every bone and hitbox, nothing else");
    assert!(
        !hung.contains(&beside),
        "a child of the rig is not under a bone"
    );
}
