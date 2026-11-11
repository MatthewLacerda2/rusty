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
//! - **Trigger.** A collider's sensor flag follows its `is_trigger`, so
//!   `Physics.SetTrigger` (or the inspector) in Play makes a door stop blocking
//!   and start reporting overlaps; the mass sync after it re-spreads the body's
//!   mass, since a trigger carries none.
//!
//! - **Mesh fit.** A concave mesh collider is the exact trimesh on a static or
//!   kinematic body and convex pieces on a dynamic one (#1886). A body that
//!   changes type in Play (`SetKinematic`, a static flag cleared) changes in
//!   place, so its concave colliders are rebuilt here, as the load would build
//!   them, and swapped onto the same collider: its handle, joints and contacts
//!   survive, and the mass sync after it re-spreads the body's mass.
//!
//! - **Mesh shape.** A mesh collider built from a library mesh (#2059) whose
//!   triangles changed (`Mesh.Create` replaced it) is rebuilt the same way, in
//!   place, by `watch::mesh_keys`; rapier wakes what loses a contact, so what
//!   rested on a plank that was halved falls.
//!
//! All walk the sorted plan, or the bodies the change log named (`watch`; a
//! step's changes reach only the body-state pass, `sync`), and touch a collider
//! only when it differs.

use rapier3d::prelude::*;

use super::compound::world_pose;
use super::shape::build::{
    class_of, collider_inputs, interaction_groups, BodyClass, EntityBodyState,
};
use super::shape::fit::{build_shape_for, fit_mark};
use super::watch::Owners;
use super::world::{collider_to_write, PhysicsWorld};
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
    pub(super) fn sync_layers(&mut self, scene: &Scene, owners: &Owners) {
        for (_, ids) in owners.pick(&self.plan) {
            for id in ids {
                let Some(&handle) = self.id_to_collider.get(id) else {
                    continue;
                };
                let layer = scene.world.layer(*id);
                let groups = interaction_groups(layer, scene.collision_matrix.filter_mask(layer));
                let differs = |c: &Collider| c.collision_groups() != groups;
                if let Some(c) = collider_to_write(&mut self.colliders, handle, differs) {
                    c.set_collision_groups(groups);
                    c.set_solver_groups(groups);
                }
            }
        }
    }

    /// Mirror each built collider's `is_trigger` onto rapier's sensor flag. A
    /// CharacterController's capsule supersedes the entity's Collider (#451) and
    /// is never a trigger, so it is left alone. Runs before
    /// [`Self::sync_masses`], which reads the flag.
    pub(super) fn sync_sensors(&mut self, scene: &Scene, owners: &Owners) {
        for (_, ids) in owners.pick(&self.plan) {
            for id in ids {
                let Some(&handle) = self.id_to_collider.get(id) else {
                    continue;
                };
                let world = &scene.world;
                let Some(is_trigger) = world.collider(*id).map(|c| c.is_trigger) else {
                    continue;
                };
                if world.character_controller(*id).is_some() {
                    continue;
                }
                let differs = |c: &Collider| c.is_sensor() != is_trigger;
                if let Some(c) = collider_to_write(&mut self.colliders, handle, differs) {
                    c.set_sensor(is_trigger);
                }
            }
        }
    }

    /// Rebuild each concave mesh collider whose fit (its `user_data`, stamped by
    /// `build_shape_for`) no longer matches its body's class: a body turned
    /// dynamic gets convex pieces, one turned static or kinematic the exact mesh.
    /// The stamp is compared before any mesh is copied, so a moving body costs a
    /// lookup per collider. A collider that is no longer a concave mesh waits for
    /// the body rebuild, like every other shape edit. A rebuild that fails keeps
    /// the old shape but takes the new stamp, so it warns once, not every step.
    /// Runs before [`Self::sync_masses`], which reads the new volume.
    pub(super) fn sync_mesh_fits(&mut self, scene: &Scene, owners: &Owners) {
        for (&owner, ids) in owners.pick(&self.plan) {
            let dynamic = matches!(class_of(&scene.world, owner), BodyClass::Dynamic);
            for id in ids {
                let Some(&handle) = self.id_to_collider.get(id) else {
                    continue;
                };
                let Some(want) = scene
                    .world
                    .collider(*id)
                    .map(|c| fit_mark(&c.shape, dynamic))
                else {
                    continue;
                };
                let differs = |c: &Collider| c.user_data != 0 && want != 0 && c.user_data != want;
                if !self.colliders.get(handle).is_some_and(differs) {
                    continue;
                }
                let built = refit(scene, *id, dynamic);
                if let Some(c) = collider_to_write(&mut self.colliders, handle, differs) {
                    if let Some(built) = built {
                        c.set_shape(built.shared_shape().clone());
                    }
                    c.user_data = want;
                }
            }
        }
    }

    /// Spread each Rigidbody owner's `mass` over its body's solid colliders by
    /// volume; a trigger carries none. An owner whose colliders have no volume
    /// (a triangle mesh) keeps rapier's own mass.
    pub(super) fn sync_masses(&mut self, scene: &Scene, owners: &Owners) {
        for (&owner, ids) in owners.pick(&self.plan) {
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
                let differs =
                    |c: &Collider| (c.mass() - target).abs() > target.max(MIN_MASS) * 1.0e-4;
                if let Some(c) = collider_to_write(&mut self.colliders, handle, differs) {
                    c.set_mass(target);
                }
            }
        }
    }
}

/// Collider entity `id`'s shape built for a body that is `dynamic` or not, or
/// `None` when it cannot be (logged by `build_shape_for`).
fn refit(scene: &Scene, id: u32, dynamic: bool) -> Option<Collider> {
    let inp = collider_inputs(&scene.world, id)?;
    let pose = world_pose(scene, id)?;
    let mesh = (inp.mesh_geom.as_ref()).map(|(p, i)| (p.as_slice(), i.as_slice()));
    build_shape_for(id, &inp.shape, pose.scale, mesh, dynamic)
}
