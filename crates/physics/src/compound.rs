//! A collider without its own Rigidbody joins the nearest ancestor that has one; with none it is
//! its own body (Unity).

use std::collections::BTreeMap;

use glam::{Quat, Vec3};

use rusty_scene::Scene;

/// Body-owner entity id -> its collider entities, the owner's own collider first.
pub(super) type BodyPlan = BTreeMap<u32, Vec<u32>>;

pub(super) use rusty_components::Pose as WorldPose;

/// A CharacterController is a body of its own, so hitboxes parented under a character ride its
/// body.
pub(super) fn body_owner(scene: &Scene, id: u32) -> u32 {
    scene.world.body_carrier(id).unwrap_or(id)
}

pub(super) fn member(scene: &Scene, id: u32) -> Option<u32> {
    let w = &scene.world;
    let collides = w.has_character_controller(id) || w.collider(id).is_some_and(|c| c.active);
    collides.then(|| body_owner(scene, id))
}

pub(super) fn plan(scene: &Scene) -> BodyPlan {
    let mut plan = BodyPlan::new();
    let world = &scene.world;
    // A character's own Collider is its capsule: listed once, with the characters.
    let colliders = world
        .ids_with_collider()
        .into_iter()
        .filter(|&id| !world.has_character_controller(id));
    for id in colliders.chain(world.ids_with_character_controller()) {
        if let Some(owner) = member(scene, id) {
            plan.entry(owner).or_default().push(id);
        }
    }
    for (&owner, ids) in plan.iter_mut() {
        ids.sort_by_key(|&id| id != owner);
    }
    plan
}

/// `id`'s world pose. A mirroring parent's reflection stays in `scale`, so `rot` × `scale` is the
/// world matrix; a root reads its `Transform` verbatim.
pub(super) fn world_pose(scene: &Scene, id: u32) -> Option<WorldPose> {
    scene.world.world_pose(id)
}

/// Under a singular parent the position stays as it was.
pub(super) fn world_to_local(scene: &Scene, id: u32, pos: Vec3, rot: Quat) -> (Vec3, Quat) {
    (
        scene.world.local_position_for(id, pos),
        scene.world.local_rotation_for(id, rot),
    )
}

pub(super) fn relative_pose(owner: &WorldPose, child: &WorldPose) -> (Vec3, Quat) {
    let inv = owner.rot.inverse();
    (inv * (child.pos - owner.pos), (inv * child.rot).normalize())
}
