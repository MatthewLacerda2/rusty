//! Mass: a Rigidbody's `mass` is spread over its body's solid colliders by volume (Unity's `Rigidbody.mass` is the whole body's).
//! Fit: a concave mesh is the exact trimesh on static/kinematic bodies, convex pieces on dynamic ones; cylinders and capsules roll only on dynamic ones.

use rapier3d::prelude::*;

use super::compound::world_pose;
use super::shape::build::{
    class_of, collider_inputs, interaction_groups, BodyClass, EntityBodyState,
};
use super::shape::fit::{build_shape_for, fit_mark};
use super::watch::Owners;
use super::world::{collider_to_write, PhysicsWorld};
use rusty_scene::Scene;

const MIN_MASS: f32 = 1.0e-4;

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

    /// A CharacterController's capsule supersedes the entity's Collider and is never a trigger.
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

    pub(super) fn sync_fits(&mut self, scene: &Scene, owners: &Owners) {
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
                        c.set_active_hooks(built.active_hooks());
                    }
                    c.user_data = want;
                }
            }
        }
    }

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

fn refit(scene: &Scene, id: u32, dynamic: bool) -> Option<Collider> {
    let inp = collider_inputs(&scene.world, id)?;
    let pose = world_pose(scene, id)?;
    let mesh = (inp.mesh_geom.as_ref()).map(|(p, i)| (p.as_slice(), i.as_slice()));
    build_shape_for(id, &inp.shape, pose.scale, mesh, dynamic)
}
