//! src/physics/compound.rs — the parent hierarchy, as physics sees it (#445).
//!
//! A `Transform` is **local** to its parent, but rapier lives in world space. This
//! module is the one place that crosses that line: it resolves an entity's world
//! pose (and converts a body's world pose back into a local `Transform`), and it
//! decides which rapier body each collider belongs to.
//!
//! **Compound colliders (Unity semantics).** A collider on an entity with its own
//! `Rigidbody` is that body's collider. A collider on an entity *without* one joins
//! the **nearest ancestor that has a Rigidbody**, as an extra collider on that body
//! offset by its pose relative to the ancestor. With no such ancestor the entity is
//! its own body, as before: static, or an implicit kinematic one.
//!
//! **Freshness.** Poses come from the ECS's world store (`ecs::world_space`), which a
//! read cleans parents first, so a transform a script wrote this tick is seen.
//!
//! **Determinism.** The plan is a `BTreeMap` keyed by owner id, and each owner's
//! colliders keep the ECS insertion order, so bodies and colliders are built — and
//! synced — in the same order every run.

use std::collections::BTreeMap;

use glam::{Quat, Vec3};

use crate::scene::Scene;

/// The body layout the scene implies: body-owner entity id → the collider
/// entities attached to that body. The owner's own collider, when it has one,
/// comes first; the rest keep insertion order.
pub(super) type BodyPlan = BTreeMap<u32, Vec<u32>>;

/// A world-space pose: translation, rotation and (lossy, signed) world scale.
/// Named apart from rapier's own `Pose`, which the physics modules also import.
pub(super) use crate::components::Pose as WorldPose;

/// The entity whose rapier body carries `id`'s collider: `id` itself when it has
/// a Rigidbody or a CharacterController, else its nearest such ancestor, else
/// `id` (its own body). A CharacterController is a body of its own (#451), so
/// hitboxes parented under a character ride its body.
pub(super) fn body_owner(scene: &Scene, id: u32) -> u32 {
    scene
        .world
        .self_and_ancestors(id)
        .find(|&c| scene.world.has_rigidbody(c) || scene.world.has_character_controller(c))
        .unwrap_or(id)
}

/// Group every active collider — and every CharacterController's capsule — under
/// the body that will carry it.
pub(super) fn plan(scene: &Scene) -> BodyPlan {
    let mut plan = BodyPlan::new();
    let world = &scene.world;
    let colliders = world
        .ids_with_collider()
        .into_iter()
        .filter(|&id| world.collider(id).is_some_and(|c| c.active))
        .filter(|&id| !world.has_character_controller(id));
    for id in colliders.chain(world.ids_with_character_controller()) {
        plan.entry(body_owner(scene, id)).or_default().push(id);
    }
    for (&owner, ids) in plan.iter_mut() {
        // Stable partition: the owner's own collider first, others in order.
        ids.sort_by_key(|&id| id != owner);
    }
    plan
}

/// `id`'s world pose ([`crate::ecs::World::world_pose`], the pose its collider's
/// world AABB reads too, #1140). A mirroring parent's
/// reflection stays in `scale` and the rotation is seen through it
/// ([`crate::components::transform::world_rotation`], #1119), so `rot` × `scale`
/// is the entity's world matrix. A root entity reads its `Transform` verbatim, so
/// the common unparented case is bit-identical to the local pose (no decompose
/// round-trip noise).
pub(super) fn world_pose(scene: &Scene, id: u32) -> Option<WorldPose> {
    scene.world.world_pose(id)
}

/// Convert a world pose into `id`'s local `Transform` position/rotation — the
/// inverse of [`world_pose`], used to write a solved body back under its parent.
/// Under a singular parent the position stays as it was (#1164).
pub(super) fn world_to_local(scene: &Scene, id: u32, pos: Vec3, rot: Quat) -> (Vec3, Quat) {
    (
        scene.world.local_position_for(id, pos),
        scene.world.local_rotation_for(id, rot),
    )
}

/// `child`'s pose expressed in `owner`'s frame (world units, scale excluded) —
/// the offset a compound collider sits at on its owner's body.
pub(super) fn relative_pose(owner: &WorldPose, child: &WorldPose) -> (Vec3, Quat) {
    let inv = owner.rot.inverse();
    (inv * (child.pos - owner.pos), (inv * child.rot).normalize())
}
