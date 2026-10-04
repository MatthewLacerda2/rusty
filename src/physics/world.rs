//! src/physics/world.rs — rapier3d-backed physics world.
//!
//! `PhysicsWorld` owns the rapier simulation state (rigid bodies, colliders, and
//! the pipeline scratch structures) and a stable-id <-> handle map. It is built
//! from the scene on entering Play and stepped once per fixed physics tick:
//!
//!   1. `sync_to_rapier`  — push the (externally mutated) component transforms and
//!      velocities into rapier. Kinematic bodies are pure movers: their next pose
//!      is their Transform (a CharacterController has already collided-and-slid
//!      it there in `Move`, see `character`); dynamic bodies get their pose,
//!      linear velocity, and gravity scale; static bodies stay put.
//!   2. `step`            — advance the rapier world by `dt` under gravity.
//!   3. `sync_from_rapier`— write the integrated transforms + velocities back onto
//!      the entities, and return the trigger and collision events scripts expect.
//!
//! Engine-glam <-> rapier-math conversion is confined to `convert`.

use crate::core::collections::Map;

use rapier3d::prelude::*;

use super::build::{body_state, gravity_scale, EntityBodyState};
use super::collision_events::CollisionEvents;
use super::compound::{world_to_local, BodyPlan};
use super::convert::{from_pose, from_rp_vec, to_pose, to_rp_vec};
use super::joints::JointMap;
use super::live::body_type;
use super::trigger_events::TriggerEvents;
use super::PhysicsEvents;
use crate::scene::Scene;

pub struct PhysicsWorld {
    gravity: Vector,
    pub(super) integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    pub(super) islands: IslandManager,
    /// Also the scene-query tree: `queries` borrows a view of it.
    pub(super) broad_phase: DefaultBroadPhase,
    pub(super) narrow_phase: NarrowPhase,
    pub(super) bodies: RigidBodySet,
    pub(super) colliders: ColliderSet,
    pub(super) impulse_joints: ImpulseJointSet,
    pub(super) multibody_joints: MultibodyJointSet,
    /// rapier's soft bodies; rusty has none, but the step takes the set.
    pub(super) soft_bodies: SoftBodySet,
    ccd_solver: CCDSolver,
    /// The body layout last built (#445): owner entity id -> the collider
    /// entities its body carries. Diffed each tick to rebuild changed bodies.
    pub(super) plan: BodyPlan,
    /// body-owner entity id -> rigid-body handle.
    pub(super) id_to_body: Map<u32, RigidBodyHandle>,
    /// collider handle -> the entity that owns the *collider* (not the body's
    /// root), so a hit on a compound reports which part was hit.
    pub(super) collider_to_id: Map<ColliderHandle, u32>,
    /// collider entity id -> its collider handle (inverse of `collider_to_id`).
    pub(super) id_to_collider: Map<u32, ColliderHandle>,
    /// Last tick's trigger-overlap pairs, diffed each step to recover the
    /// enter/exit edges (#310). Starts empty, so a play session's first
    /// overlapping tick is an "enter".
    prev_triggers: Vec<(u32, u32)>,
    /// Last tick's touching solid pairs, diffed the same way for the
    /// `OnCollision*` edges (#448).
    prev_collisions: Vec<(u32, u32)>,
    /// Joint entity id -> its live rapier joint (#449, see `joints`).
    pub(super) joints: JointMap,
    /// The fixed body world-anchored joints attach to, made on first use.
    pub(super) world_body: Option<RigidBodyHandle>,
}

impl PhysicsWorld {
    /// Build the rapier world from the current scene: one rigid body per plan
    /// owner, carrying every collider attached to it (see `compound`).
    pub fn from_scene(scene: &Scene) -> Self {
        let mut world = Self {
            gravity: Vector::new(0.0, -9.81, 0.0),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            soft_bodies: SoftBodySet::new(),
            ccd_solver: CCDSolver::new(),
            plan: BodyPlan::new(),
            id_to_body: Map::default(),
            collider_to_id: Map::default(),
            id_to_collider: Map::default(),
            prev_triggers: Vec::new(),
            prev_collisions: Vec::new(),
            joints: JointMap::new(),
            world_body: None,
        };
        world.build_bodies(scene);
        world.resync_joints(scene);
        world.sync_enabled(scene);
        // Prime the query tree so a raycast works before the first step.
        let all: Vec<ColliderHandle> = world.colliders.iter().map(|(h, _)| h).collect();
        world.refresh_query_tree(&all);
        world
    }

    /// Push current component state into rapier before stepping: first rebuild
    /// any body whose collider set changed, then every enabled flag (before any
    /// character sweep, so no sweep sees a stale one), then each owner's world
    /// pose and velocities and each compound collider's offset. Walks the sorted
    /// plan, so the order is deterministic.
    fn sync_to_rapier(&mut self, scene: &Scene) {
        self.resync_topology(scene);
        self.resync_joints(scene);
        self.sync_enabled(scene);
        self.sync_materials(scene);
        self.sync_layers(scene);
        self.sync_masses(scene);
        self.sync_characters(scene);
        let plan = std::mem::take(&mut self.plan);
        for (&owner, ids) in &plan {
            let Some(&handle) = self.id_to_body.get(&owner) else {
                continue;
            };
            let Some(snapshot) = body_state(scene, owner) else {
                continue;
            };
            self.apply_body_state(handle, &snapshot);
            self.sync_compound_parts(scene, owner, ids);
        }
        self.plan = plan;
    }

    /// Push one entity's snapshot into its rapier body for this tick.
    fn apply_body_state(&mut self, handle: RigidBodyHandle, snap: &EntityBodyState) {
        let body = &mut self.bodies[handle];
        let class = body_type(snap);
        if body.body_type() != class {
            // A Rigidbody switched class mid-play (`SetKinematic`, a ragdoll
            // handoff, #466): the body changes in place at the entity's current
            // pose, so its colliders, joints and contacts survive.
            body.set_body_type(class, true);
            body.set_position(to_pose(snap.pos, snap.rot), true);
        }
        // Re-apply the CCD mode each tick so `Physics.SetCollisionDetection`
        // toggled mid-play takes effect (mirrors the `gravity_scale` re-apply).
        body.enable_ccd(snap.ccd_enabled);
        if snap.kinematic {
            // A pure mover (Unity's kinematic Rigidbody): it goes exactly where
            // its Transform says, through anything in the way.
            body.set_next_kinematic_position(to_pose(snap.pos, snap.rot));
        } else if snap.is_static {
            body.set_position(to_pose(snap.pos, snap.rot), true);
        } else {
            // Dynamic: trust rapier for pose, but let scripts inject linear and
            // angular velocity (SetVelocity / SetAngularVelocity / AddForce mutate
            // the component between ticks) and re-apply `use_gravity` so toggling
            // it at runtime takes effect.
            body.set_linvel(to_rp_vec(snap.vel), true);
            body.set_angvel(to_rp_vec(snap.angular_velocity), true);
            let scale = gravity_scale(snap.use_gravity);
            if body.gravity_scale() != scale {
                // A full wake: rapier's setter only clears the sleep flag, so a
                // body that slept while weightless would doze straight off again.
                body.set_gravity_scale(scale, false);
                body.wake_up(true);
            }
        }
    }

    /// Advance the rapier world by `dt` and surface the tick's trigger and
    /// solid-contact events — enter/stay/exit distinctly (#310, #448) — and the
    /// joints that broke (#449), each list sorted for deterministic dispatch.
    pub fn step(&mut self, scene: &mut Scene, dt: f32) -> PhysicsEvents {
        self.integration_parameters.dt = dt;
        self.sync_to_rapier(scene);
        let pre_solve = self.snapshot_velocities();

        self.physics_pipeline.step(
            self.gravity,
            &self.integration_parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.soft_bodies,
            &mut self.ccd_solver,
            &(),
            &(),
        );

        let overlaps = self.overlap_pairs();
        let triggers = TriggerEvents::from_overlap_sets(&self.prev_triggers, overlaps);
        self.prev_triggers = triggers.stayed.clone();
        let contacts = self.collect_contact_pairs(&pre_solve);
        let collisions = CollisionEvents::from_contact_sets(&self.prev_collisions, contacts);
        self.prev_collisions = collisions.stayed_keys();
        let joint_breaks = self.break_joints(scene, dt);
        self.sync_from_rapier(scene);
        PhysicsEvents {
            triggers,
            collisions,
            joint_breaks,
        }
    }

    /// Write integrated poses + velocities back onto the owner entities,
    /// converting each body's world pose into the owner's local `Transform`
    /// (#445). Static and kinematic bodies keep their transform: physics never
    /// moves the one, and the other is where its Transform sent it (no
    /// world↔local round-trip drift). Every
    /// attached collider's world AABB is refreshed, since moving an owner moves
    /// its compound children too.
    fn sync_from_rapier(&self, scene: &mut Scene) {
        for &owner in self.plan.keys() {
            self.write_back(scene, owner);
        }
    }

    /// Write one owner's integrated pose and velocities back onto its entity and
    /// refresh the world AABB of every collider its body carries.
    pub(super) fn write_back(&self, scene: &mut Scene, owner: u32) {
        let (Some(ids), Some(body)) = (
            self.plan.get(&owner),
            self.id_to_body
                .get(&owner)
                .and_then(|&h| self.bodies.get(h)),
        ) else {
            return;
        };
        // Only a dynamic body's pose is the solver's. A kinematic one ended the
        // step where its Transform put it, so writing it back would only add
        // world↔local rounding to every animated bone each tick.
        if body.is_dynamic() {
            let (pos, rot) = from_pose(body.position());
            let (local_pos, local_rot) = world_to_local(scene, owner, pos, rot);
            if let Some(mut t) = scene.world.transform_mut(owner) {
                t.position = local_pos;
                t.rotation = local_rot;
            }
        }
        if let Some(mut rb) = scene.world.rigidbody_mut(owner) {
            if !rb.is_kinematic {
                rb.velocity = from_rp_vec(body.linvel());
                rb.angular_velocity = from_rp_vec(body.angvel());
            }
        }
        for &id in ids {
            scene.update_entity_collider(id);
        }
    }
}
