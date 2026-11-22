use rapier3d::prelude::*;

use super::character::upright_offset;
use super::compound::{plan, relative_pose, world_pose, BodyPlan, WorldPose};
use super::convert::to_pose;
use super::material::apply_material;
use super::shape::build::{
    body_inputs, ccd_enabled, collider_inputs, gravity_scale, interaction_groups, BodyClass,
};
use super::shape::fit::build_shape_for;
use super::watch::{carriers, Owners};
use super::world::{body_to_write, collider_to_write, PhysicsWorld};
use rusty_scene::Scene;

/// Only pairs with a dynamic body generate contacts: Unity's rule that a collision needs a dynamic
/// body; kinematic and static colliders meet only through queries. Equals
/// `ActiveCollisionTypes::default()` (not a `const fn`; see `active_types_are_rapiers_default`).
/// rapier re-filters existing pairs only through its broad phase, so a body type change in Play
/// re-pairs its colliders.
pub(crate) const ACTIVE_TYPES: ActiveCollisionTypes = ActiveCollisionTypes::DYNAMIC_DYNAMIC
    .union(ActiveCollisionTypes::DYNAMIC_KINEMATIC)
    .union(ActiveCollisionTypes::DYNAMIC_FIXED);

impl PhysicsWorld {
    /// Rebuilds only the owners whose collider set changed, so the rest keep their contacts and
    /// sleep; in ascending owner order, so rebuild order is deterministic. Returns those built.
    pub(super) fn resync_topology(&mut self, scene: &Scene) -> Vec<u32> {
        let next = plan(scene);
        if next == self.plan {
            return Vec::new();
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
        let mut built = Vec::new();
        for (&owner, ids) in &next {
            if self.plan.get(&owner) != Some(ids) {
                self.build_owner(scene, owner, ids);
                built.push(owner);
            }
        }
        self.tracked.remember_plan(&next);
        self.plan = next;
        self.record_mesh_keys(scene, &built);
        built
    }

    pub(super) fn build_bodies(&mut self, scene: &Scene) {
        self.plan = BodyPlan::new();
        self.resync_topology(scene);
        self.tracked.carriers = carriers(scene);
    }

    fn remove_owner(&mut self, owner: u32) {
        let Some(handle) = self.id_to_body.remove(&owner) else {
            return;
        };
        self.body_to_id.remove(&handle);
        self.tracked.static_poses.remove(&handle);
        if let Some(body) = self.bodies.get(handle) {
            for collider in body.colliders() {
                if let Some(id) = self.collider_to_id.remove(collider) {
                    self.id_to_collider.remove(&id);
                }
            }
        }
        self.remove_body(handle);
    }

    fn build_owner(&mut self, scene: &Scene, owner: u32, ids: &[u32]) {
        let Some(owner_pose) = world_pose(scene, owner) else {
            return;
        };
        let inp = body_inputs(&scene.world, owner);
        let dynamic = matches!(inp.class, BodyClass::Dynamic);
        let parts: Vec<(u32, Collider)> = ids
            .iter()
            .filter_map(|&id| Some((id, self.build_collider(scene, &owner_pose, id, dynamic)?)))
            .collect();
        if parts.is_empty() {
            return;
        }
        let body_builder = match inp.class {
            BodyClass::Static => RigidBodyBuilder::fixed(),
            BodyClass::Kinematic => RigidBodyBuilder::kinematic_position_based(),
            BodyClass::Dynamic => RigidBodyBuilder::dynamic()
                .linvel(inp.velocity)
                .angvel(inp.angular_velocity)
                .gravity_scale(gravity_scale(inp.use_gravity))
                .linear_damping(inp.damping.linear)
                .angular_damping(inp.damping.angular),
        }
        .ccd_enabled(ccd_enabled(inp.collision_detection))
        .pose(to_pose(owner_pose.pos, owner_pose.rot));
        let body_handle = self.insert_body(body_builder.build());
        for (id, collider) in parts {
            let handle = self
                .colliders
                .insert_with_parent(collider, body_handle, &mut self.bodies);
            self.collider_to_id.insert(handle, id);
            self.id_to_collider.insert(id, handle);
        }
        self.id_to_body.insert(owner, body_handle);
        self.body_to_id.insert(body_handle, owner);
    }

    pub(super) fn build_collider(
        &self,
        scene: &Scene,
        owner_pose: &WorldPose,
        id: u32,
        dynamic: bool,
    ) -> Option<Collider> {
        let inp = collider_inputs(&scene.world, id)?;
        let pose = world_pose(scene, id)?;
        let mesh_ref = inp
            .mesh_geom
            .as_ref()
            .map(|(p, i)| (p.as_slice(), i.as_slice()));
        let mut collider = build_shape_for(id, &inp.shape, pose.scale, mesh_ref, dynamic)?;
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
        collider.set_enabled(scene.world.is_active_in_hierarchy(id));
        collider.set_sensor(inp.is_trigger);
        apply_material(&mut collider, &inp.material);
        collider.set_active_events(ActiveEvents::COLLISION_EVENTS);
        collider.set_active_collision_types(ACTIVE_TYPES);
        let groups = interaction_groups(inp.layer, scene.collision_matrix.filter_mask(inp.layer));
        // Both groups alike, so contact generation and the solver agree.
        collider.set_collision_groups(groups);
        collider.set_solver_groups(groups);
        Some(collider)
    }

    pub(super) fn sync_enabled(&mut self, scene: &Scene, owners: &Owners) {
        for (&owner, ids) in owners.pick(&self.plan) {
            let on = scene.world.is_active_in_hierarchy(owner);
            let body = self.id_to_body.get(&owner);
            let body =
                body.and_then(|&h| body_to_write(&mut self.bodies, h, |b| b.is_enabled() != on));
            if let Some(body) = body {
                body.set_enabled(on);
            }
            for &id in ids {
                let on = scene.world.is_active_in_hierarchy(id);
                let handle = self.id_to_collider.get(&id);
                let differs = |c: &Collider| c.is_enabled() != on;
                if let Some(c) =
                    handle.and_then(|&h| collider_to_write(&mut self.colliders, h, differs))
                {
                    c.set_enabled(on);
                }
            }
        }
    }

    pub(super) fn sync_compound_parts(&mut self, scene: &Scene, owner: u32, ids: &[u32]) {
        let Some(owner_pose) = world_pose(scene, owner) else {
            return;
        };
        for &id in ids.iter().filter(|&&id| id != owner) {
            let (Some(&handle), Some(pose)) = (self.id_to_collider.get(&id), world_pose(scene, id))
            else {
                continue;
            };
            let (pos, rot) = relative_pose(&owner_pose, &pose);
            let offset = to_pose(pos, rot);
            let differs = |c: &Collider| c.position_wrt_parent() != Some(&offset);
            if let Some(c) = collider_to_write(&mut self.colliders, handle, differs) {
                c.set_position_wrt_parent(offset);
            }
        }
    }
}
