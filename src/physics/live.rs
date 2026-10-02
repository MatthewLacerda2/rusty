//! src/physics/live.rs — component edits made mid-play that reach built bodies.
//!
//! A body is built once and then only synced, so every property a script or the
//! editor may change during Play needs a path into rapier. The pose, velocities,
//! gravity and CCD flag go through `PhysicsWorld::apply_body_state`; the body
//! class (`SetKinematic`, a ragdoll handoff — #466) is switched there too, from
//! [`body_type`]. This file holds the per-collider rest:
//!
//! - **Layer.** A collider's collision and solver groups follow its entity's
//!   layer and the collision matrix, so moving a hitbox onto the ragdoll layer
//!   makes it collide at once.
//! - **Mass.** A Rigidbody's `mass` is spread over its body's solid colliders in
//!   proportion to their volume (Unity's `Rigidbody.mass` is the body's mass,
//!   whatever its colliders' size), so a compound body keeps a sensible centre of
//!   mass and inertia, and a ragdoll's per-bone masses are what the builder set.
//!
//! Both walk the sorted plan and touch a collider only when it differs, so an
//! unchanged scene costs comparisons, not rapier change-tracking.

use rapier3d::prelude::*;

use super::build::{interaction_groups, EntityBodyState};
use super::world::PhysicsWorld;
use crate::scene::Scene;

/// The lightest mass a body may be given, so a zero never divides.
const MIN_MASS: f32 = 1.0e-4;

/// The rapier body type an owner's snapshot calls for.
pub(super) fn body_type(snap: &EntityBodyState) -> RigidBodyType {
    if snap.is_static {
        RigidBodyType::Fixed
    } else if snap.kinematic {
        RigidBodyType::KinematicPositionBased
    } else {
        RigidBodyType::Dynamic
    }
}

impl PhysicsWorld {
    /// Re-derive each built collider's groups from its entity's layer and the
    /// collision matrix.
    pub(super) fn sync_layers(&mut self, scene: &Scene) {
        for ids in self.plan.values() {
            for id in ids {
                let Some(&handle) = self.id_to_collider.get(id) else {
                    continue;
                };
                let layer = scene.world.layer(*id);
                let groups = interaction_groups(layer, scene.collision_matrix.filter_mask(layer));
                if self.colliders.get(handle).map(|c| c.collision_groups()) == Some(groups) {
                    continue;
                }
                if let Some(c) = self.colliders.get_mut(handle) {
                    c.set_collision_groups(groups);
                    c.set_solver_groups(groups);
                }
            }
        }
    }

    /// Spread each Rigidbody owner's `mass` over its body's solid colliders by
    /// volume; a trigger carries none. An owner whose colliders have no volume
    /// (a triangle mesh) keeps rapier's own mass.
    pub(super) fn sync_masses(&mut self, scene: &Scene) {
        for (&owner, ids) in &self.plan {
            let Some(mass) = scene.world.rigidbody(owner).map(|rb| rb.mass.max(MIN_MASS)) else {
                continue;
            };
            let parts: Vec<(ColliderHandle, f32)> = ids
                .iter()
                .filter_map(|id| self.id_to_collider.get(id).copied())
                .filter_map(|h| {
                    let c = self.colliders.get(h)?;
                    Some((h, if c.is_sensor() { 0.0 } else { c.volume() }))
                })
                .collect();
            let total: f32 = parts.iter().map(|&(_, v)| v).sum();
            if total <= 0.0 {
                continue;
            }
            for (handle, volume) in parts {
                let target = mass * volume / total;
                let current = self.colliders.get(handle).map_or(target, |c| c.mass());
                if (current - target).abs() <= target.max(MIN_MASS) * 1.0e-4 {
                    continue;
                }
                if let Some(c) = self.colliders.get_mut(handle) {
                    c.set_mass(target);
                }
            }
        }
    }
}
