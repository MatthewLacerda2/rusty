//! src/physics/assemble.rs — turning the scene's body plan into rapier bodies.
//!
//! One rapier body per plan owner (see `compound`), carrying one collider per
//! attached collider entity, each at its pose relative to the owner. The plan is
//! re-derived every tick; an owner whose collider set changed (reparent, a
//! collider or Rigidbody added/removed, an entity spawned or destroyed) has its
//! body rebuilt, the others are left untouched so their contacts and sleep state
//! survive.

use rapier3d::prelude::*;

use super::build::{
    body_inputs, build_shape, ccd_enabled, collider_inputs, gravity_scale, interaction_groups,
    BodyClass,
};
use super::character::upright_offset;
use super::compound::{plan, relative_pose, world_pose, BodyPlan, WorldPose};
use super::convert::to_pose;
use super::material::apply_material;
use super::world::PhysicsWorld;
use crate::scene::Scene;

impl PhysicsWorld {
    /// Re-derive the body plan and rebuild exactly the owners whose collider set
    /// changed since the last build. Iterates the sorted plans, so rebuild order
    /// is deterministic.
    pub(super) fn resync_topology(&mut self, scene: &Scene) {
        let next = plan(scene);
        if next == self.plan {
            return;
        }
        let stale: Vec<u32> = self
            .plan
            .iter()
            .filter(|(owner, ids)| next.get(owner) != Some(ids))
            .map(|(&owner, _)| owner)
            .collect();
        for owner in stale {
            self.remove_owner(owner);
        }
        for (&owner, ids) in &next {
            if self.plan.get(&owner) != Some(ids) {
                self.build_owner(scene, owner, ids);
            }
        }
        self.plan = next;
    }

    /// Build every body the scene implies (the initial build on entering Play).
    pub(super) fn build_bodies(&mut self, scene: &Scene) {
        self.plan = BodyPlan::new();
        self.resync_topology(scene);
    }

    /// Drop `owner`'s body and every collider attached to it.
    fn remove_owner(&mut self, owner: u32) {
        let Some(handle) = self.id_to_body.remove(&owner) else {
            return;
        };
        if let Some(body) = self.bodies.get(handle) {
            for collider in body.colliders() {
                if let Some(id) = self.collider_to_id.remove(collider) {
                    self.id_to_collider.remove(&id);
                }
            }
        }
        self.bodies.remove(
            handle,
            &mut self.islands,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            true,
        );
    }

    /// Build `owner`'s body with the colliders of `ids` attached. An owner none
    /// of whose colliders can be built (degenerate mesh geometry) gets no body.
    fn build_owner(&mut self, scene: &Scene, owner: u32, ids: &[u32]) {
        let Some(owner_pose) = world_pose(scene, owner) else {
            return;
        };
        let parts: Vec<(u32, Collider)> = ids
            .iter()
            .filter_map(|&id| Some((id, self.build_collider(scene, &owner_pose, id)?)))
            .collect();
        if parts.is_empty() {
            return;
        }
        let inp = body_inputs(&scene.world, owner);
        let body_builder = match inp.class {
            BodyClass::Static => RigidBodyBuilder::fixed(),
            BodyClass::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            BodyClass::Dynamic => RigidBodyBuilder::dynamic()
                .linvel(inp.velocity)
                .angvel(inp.angular_velocity)
                // `use_gravity = false` exempts the body from world gravity (#209).
                .gravity_scale(gravity_scale(inp.use_gravity)),
        }
        // Continuous mode switches on rapier's CCD sweep (anti-tunnelling, #321);
        // Discrete leaves it off. Honoured for every class — mainly dynamic, but a
        // kinematic body may opt in to be swept against dynamic bodies.
        .ccd_enabled(ccd_enabled(inp.collision_detection))
        .pose(to_pose(owner_pose.pos, owner_pose.rot));
        let body_handle = self.bodies.insert(body_builder.build());
        for (id, collider) in parts {
            let handle = self
                .colliders
                .insert_with_parent(collider, body_handle, &mut self.bodies);
            self.collider_to_id.insert(handle, id);
            self.id_to_collider.insert(id, handle);
        }
        self.id_to_body.insert(owner, body_handle);
    }

    /// Build collider entity `id`'s rapier collider, positioned relative to its
    /// owner's pose. `None` for a dead entity or a degenerate mesh.
    fn build_collider(&self, scene: &Scene, owner_pose: &WorldPose, id: u32) -> Option<Collider> {
        let inp = collider_inputs(&scene.world, id)?;
        let pose = world_pose(scene, id)?;
        let mesh_ref = inp
            .mesh_geom
            .as_ref()
            .map(|(p, i)| (p.as_slice(), i.as_slice()));
        let mut collider = build_shape(&inp.shape, pose.scale, mesh_ref)?;
        let (offset_pos, offset_rot) = relative_pose(owner_pose, &pose);
        let offset = if inp.upright {
            upright_offset(
                owner_pose.rot,
                offset_pos,
                offset_rot,
                inp.center * pose.scale,
            )
        } else {
            to_pose(offset_pos, offset_rot)
        };
        collider.set_position(offset);
        collider.set_enabled(scene.world.is_active(id));
        collider.set_sensor(inp.is_trigger);
        apply_material(&mut collider, &inp.material);
        collider.set_active_events(ActiveEvents::COLLISION_EVENTS);
        // The demo's bodies are kinematic/static, so the default
        // "only-if-one-is-dynamic" filtering would suppress every player↔wall
        // and player↔enemy pair. Enable all type combinations.
        collider.set_active_collision_types(ActiveCollisionTypes::all());
        // The collision matrix (#91) decides which layers interact: a collider
        // is a member of its own layer and filters to the layers it may collide
        // with. Set on both collision and solver groups so contact generation
        // and the solver agree.
        let groups = interaction_groups(inp.layer, scene.collision_matrix.filter_mask(inp.layer));
        collider.set_collision_groups(groups);
        collider.set_solver_groups(groups);
        Some(collider)
    }

    /// Mirror activation onto rapier's enabled flags: each body follows its
    /// owner entity, each collider (the owner's own included) its own entity. A
    /// disabled body or collider generates no contacts, and [`super::query::is_live`]
    /// hides it from every query (#521).
    pub(super) fn sync_enabled(&mut self, scene: &Scene) {
        for (&owner, ids) in &self.plan {
            if let Some(body) = self
                .id_to_body
                .get(&owner)
                .and_then(|&h| self.bodies.get_mut(h))
            {
                body.set_enabled(scene.world.is_active(owner));
            }
            for &id in ids {
                let handle = self.id_to_collider.get(&id);
                if let Some(collider) = handle.and_then(|&h| self.colliders.get_mut(h)) {
                    collider.set_enabled(scene.world.is_active(id));
                }
            }
        }
    }

    /// Keep each compound collider (one not on its owner entity) at its current
    /// pose relative to the owner — so a script or the animator moving a child
    /// hitbox moves the collider with it.
    pub(super) fn sync_compound_parts(&mut self, scene: &Scene, owner: u32, ids: &[u32]) {
        let Some(owner_pose) = world_pose(scene, owner) else {
            return;
        };
        for &id in ids.iter().filter(|&&id| id != owner) {
            let (Some(&handle), Some(pose)) = (self.id_to_collider.get(&id), world_pose(scene, id))
            else {
                continue;
            };
            let Some(collider) = self.colliders.get_mut(handle) else {
                continue;
            };
            let (pos, rot) = relative_pose(&owner_pose, &pose);
            let offset = to_pose(pos, rot);
            if collider.position_wrt_parent() != Some(&offset) {
                collider.set_position_wrt_parent(offset);
            }
        }
    }
}
