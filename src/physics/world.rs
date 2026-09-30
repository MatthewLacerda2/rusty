//! src/physics/world.rs — rapier3d-backed physics world.
//!
//! `PhysicsWorld` owns the rapier simulation state (rigid bodies, colliders, and
//! the pipeline scratch structures) and a stable-id <-> handle map. It is built
//! from the scene on entering Play and stepped once per fixed physics tick:
//!
//!   1. `sync_to_rapier`  — push the (externally mutated) component transforms and
//!      velocities into rapier. Kinematic bodies (player/enemy) are routed through
//!      the `KinematicCharacterController` (collide-and-slide vs. walls, plus the
//!      gravity fall speed when `use_gravity` is authored; see `character`) and
//!      given the corrected next pose; dynamic bodies get their pose, linear
//!      velocity, and gravity scale; static bodies stay put.
//!   2. `step`            — advance the rapier world by `dt` under gravity.
//!   3. `sync_from_rapier`— write the integrated transforms + velocities back onto
//!      the entities, and return the trigger/collision pairs scripts expect.
//!
//! glam <-> nalgebra conversion is confined to `convert`; the engine stays glam.

use std::collections::HashMap;

use rapier3d::prelude::*;

use super::build::{body_state, gravity_scale, EntityBodyState};
use super::character;
use super::compound::{world_to_local, BodyPlan};
use super::convert::{from_iso, from_na_vec, to_iso, to_na_vec};
use super::trigger_events::{self, TriggerEvents};
use crate::scene::Scene;

pub struct PhysicsWorld {
    gravity: Vector<Real>,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    pub(super) islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    pub(super) bodies: RigidBodySet,
    pub(super) colliders: ColliderSet,
    pub(super) impulse_joints: ImpulseJointSet,
    pub(super) multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
    /// Exposed to the `physics` module (see `query`) for ray casts.
    pub(super) query_pipeline: QueryPipeline,
    /// Per-body downward fall speed for gravity-driven kinematic bodies (#318),
    /// keyed by entity id and carried across ticks; zeroed on ground contact.
    fall_speeds: HashMap<u32, f32>,
    /// The body layout last built (#445): owner entity id -> the collider
    /// entities its body carries. Diffed each tick to rebuild changed bodies.
    pub(super) plan: BodyPlan,
    /// body-owner entity id -> rigid-body handle.
    pub(super) id_to_body: HashMap<u32, RigidBodyHandle>,
    /// collider handle -> the entity that owns the *collider* (not the body's
    /// root), so a hit on a compound reports which part was hit.
    pub(super) collider_to_id: HashMap<ColliderHandle, u32>,
    /// collider entity id -> its collider handle (inverse of `collider_to_id`).
    pub(super) id_to_collider: HashMap<u32, ColliderHandle>,
    /// collider entity id -> its trigger flag (sensors surface trigger pairs).
    pub(super) id_is_trigger: HashMap<u32, bool>,
    /// Last tick's trigger-overlap pairs, diffed each step to recover the
    /// enter/exit edges (#310). Starts empty, so a play session's first
    /// overlapping tick is an "enter".
    prev_triggers: Vec<(u32, u32)>,
}

impl PhysicsWorld {
    /// Build the rapier world from the current scene: one rigid body per plan
    /// owner, carrying every collider attached to it (see `compound`).
    pub fn from_scene(scene: &Scene) -> Self {
        let mut world = Self {
            gravity: vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            fall_speeds: HashMap::new(),
            plan: BodyPlan::new(),
            id_to_body: HashMap::new(),
            collider_to_id: HashMap::new(),
            id_to_collider: HashMap::new(),
            id_is_trigger: HashMap::new(),
            prev_triggers: Vec::new(),
        };
        world.build_bodies(scene);
        // Prime the query pipeline so a raycast works before the first step.
        world.query_pipeline.update(&world.bodies, &world.colliders);
        world
    }

    /// Push current component state into rapier before stepping: first rebuild
    /// any body whose collider set changed, then each owner's world pose and
    /// velocities, then each compound collider's offset. Walks the sorted plan,
    /// so the order is deterministic.
    fn sync_to_rapier(&mut self, scene: &Scene, dt: f32) {
        self.resync_topology(scene);
        let plan = std::mem::take(&mut self.plan);
        for (&owner, ids) in &plan {
            let Some(&handle) = self.id_to_body.get(&owner) else {
                continue;
            };
            let Some(snapshot) = body_state(scene, owner) else {
                continue;
            };
            self.apply_body_state(owner, handle, &snapshot, dt);
            self.sync_compound_parts(scene, owner, ids);
        }
        self.plan = plan;
    }

    /// Push one entity's snapshot into its rapier body for this tick.
    fn apply_body_state(
        &mut self,
        id: u32,
        handle: RigidBodyHandle,
        snap: &EntityBodyState,
        dt: f32,
    ) {
        if snap.kinematic {
            // Route the script/input-set move through the controller so the
            // body collides-and-slides against walls instead of teleporting.
            // With `use_gravity` authored, feed the accumulated fall speed into
            // the move (#318) — rapier never gravity-integrates a kinematic body.
            let gravity = (snap.active && snap.use_gravity).then(|| character::GravityFall {
                accel: -self.gravity.y,
                speed: self.fall_speeds.get(&id).copied().unwrap_or(0.0),
            });
            let (next, fall_speed) = character::corrected_next_pose(
                character::RapierRefs {
                    bodies: &self.bodies,
                    colliders: &self.colliders,
                    queries: &self.query_pipeline,
                },
                handle,
                to_iso(snap.pos, snap.rot),
                dt,
                gravity,
            );
            self.fall_speeds.insert(id, fall_speed);
            let body = &mut self.bodies[handle];
            body.set_enabled(snap.active);
            body.enable_ccd(snap.ccd_enabled);
            body.set_next_kinematic_position(next);
            return;
        }
        // Leaving the kinematic class (`Physics.SetKinematic`) drops any carried
        // fall speed, so toggling back later starts a fresh fall.
        self.fall_speeds.remove(&id);
        let body = &mut self.bodies[handle];
        body.set_enabled(snap.active);
        // Re-apply the CCD mode each tick so `Physics.SetCollisionDetection`
        // toggled mid-play takes effect (mirrors the `gravity_scale` re-apply).
        body.enable_ccd(snap.ccd_enabled);
        if snap.is_static {
            body.set_position(to_iso(snap.pos, snap.rot), true);
        } else {
            // Dynamic: trust rapier for pose, but let scripts inject linear and
            // angular velocity (SetVelocity / SetAngularVelocity / AddForce mutate
            // the component between ticks) and re-apply `use_gravity` so toggling
            // it at runtime takes effect.
            body.set_linvel(to_na_vec(snap.vel), true);
            body.set_angvel(to_na_vec(snap.angular_velocity), true);
            body.set_gravity_scale(gravity_scale(snap.use_gravity), true);
        }
    }

    /// Advance the rapier world by `dt` and surface the tick's trigger events —
    /// enter/stay/exit distinctly (#310), each list sorted for deterministic
    /// dispatch.
    pub fn step(&mut self, scene: &mut Scene, dt: f32) -> TriggerEvents {
        self.integration_parameters.dt = dt;
        self.sync_to_rapier(scene, dt);

        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &(),
            &(),
        );

        let current = trigger_events::collect_overlap_pairs(
            &self.narrow_phase,
            &self.collider_to_id,
            &self.id_is_trigger,
        );
        let events = TriggerEvents::from_overlap_sets(&self.prev_triggers, current);
        self.prev_triggers = events.stayed.clone();
        self.sync_from_rapier(scene);
        events
    }

    /// Write integrated poses + velocities back onto the owner entities,
    /// converting each body's world pose into the owner's local `Transform`
    /// (#445). Static bodies are skipped: physics never moves them, so their
    /// transform stays authoritative (no world↔local round-trip drift). Every
    /// attached collider's world AABB is refreshed, since moving an owner moves
    /// its compound children too.
    fn sync_from_rapier(&self, scene: &mut Scene) {
        for (&owner, ids) in &self.plan {
            let Some(body) = self
                .id_to_body
                .get(&owner)
                .and_then(|&h| self.bodies.get(h))
            else {
                continue;
            };
            if !body.is_fixed() {
                let (pos, rot) = from_iso(body.position());
                let (local_pos, local_rot) = world_to_local(scene, owner, pos, rot);
                if let Some(mut t) = scene.world.transform_mut(owner) {
                    t.position = local_pos;
                    t.rotation = local_rot;
                }
            }
            if let Some(mut rb) = scene.world.rigidbody_mut(owner) {
                if !rb.is_kinematic {
                    rb.velocity = from_na_vec(*body.linvel());
                    rb.angular_velocity = from_na_vec(*body.angvel());
                }
            }
            for &id in ids {
                scene.update_entity_collider(id);
            }
        }
    }
}
