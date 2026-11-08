//! src/physics/world.rs — rapier3d-backed physics world.
//!
//! `PhysicsWorld` owns the rapier simulation state (rigid bodies, colliders, and
//! the pipeline scratch structures) and a stable-id <-> handle map. It is built
//! from the scene on entering Play and stepped once per fixed physics tick:
//!
//!   1. `sync_to_rapier`  — push the (externally mutated) component transforms and
//!      velocities into rapier, for the bodies the World's change log names and,
//!      for their pose and velocities, the bodies the last step may have changed
//!      (`sync`, `watch`; every body on the first step, a load or an Undo). Kinematic bodies are pure movers: their next pose
//!      is their Transform (a CharacterController has already collided-and-slid
//!      it there in `Move`, see `character`); dynamic bodies get their pose,
//!      linear velocity, and gravity scale; static bodies stay put.
//!   2. `step`            — advance the rapier world by `dt` under gravity.
//!   3. `sync_from_rapier`— write the integrated transforms + velocities back onto
//!      the entities, and return the trigger and collision events scripts expect.
//!
//! The world is derived state: `recover` rebuilds it from the scene when rapier
//! panics in a release build (#1797), and holds back rapier's pipeline on a
//! zero-length step before the world's first real one (#1795).
//!
//! Engine-glam <-> rapier-math conversion is confined to `convert`.

use crate::core::collections::Map;

use rapier3d::prelude::*;

use super::compound::{world_to_local, BodyPlan};
use super::convert::{from_pose, to_pose};
use super::events::collision::CollisionEvents;
use super::events::trigger::TriggerEvents;
use super::joints::JointMap;
use super::live::body_type;
use super::recover::Recovery;
use super::shape::build::{gravity_scale, EntityBodyState};
use super::watch::{Owners, Tracked};
pub(super) use super::writes::{body_to_write, collider_to_write, state_differs};
use super::PhysicsEvents;
use crate::scene::Scene;

pub struct PhysicsWorld {
    pub(super) gravity: Vector,
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
    /// entities its body carries. Re-derived and diffed to rebuild changed bodies.
    pub(super) plan: BodyPlan,
    /// body-owner entity id -> rigid-body handle.
    pub(super) id_to_body: Map<u32, RigidBodyHandle>,
    /// rigid-body handle -> its owner entity (inverse of `id_to_body`).
    pub(super) body_to_id: Map<RigidBodyHandle, u32>,
    /// collider handle -> the entity that owns the *collider* (not the body's
    /// root), so a hit on a compound reports which part was hit.
    pub(super) collider_to_id: Map<ColliderHandle, u32>,
    /// collider entity id -> its collider handle (inverse of `collider_to_id`).
    pub(super) id_to_collider: Map<u32, ColliderHandle>,
    /// Last tick's trigger-overlap pairs, diffed each step to recover the
    /// enter/exit edges (#310). Starts empty, so a play session's first
    /// overlapping tick is an "enter".
    pub(super) prev_triggers: Vec<(u32, u32)>,
    /// Last tick's touching solid pairs, diffed the same way for the
    /// `OnCollision*` edges (#448).
    pub(super) prev_collisions: Vec<(u32, u32)>,
    /// Joint entity id -> its live rapier joint (#449, see `joints`).
    pub(super) joints: JointMap,
    /// The fixed body world-anchored joints attach to, made on first use.
    pub(super) world_body: Option<RigidBodyHandle>,
    /// The joints whose motors the last step drove (#1895, `joints::drive`).
    pub(super) driven: std::collections::BTreeMap<u32, super::joints::drive::MotorSet>,
    /// What the sync before each step needs to sync only what changed (#1707).
    pub(super) tracked: Tracked,
    /// What the world remembers about its own steps (#1795, #1797, see `recover`).
    pub(super) recovery: Recovery,
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
            body_to_id: Map::default(),
            collider_to_id: Map::default(),
            id_to_collider: Map::default(),
            prev_triggers: Vec::new(),
            prev_collisions: Vec::new(),
            joints: JointMap::new(),
            world_body: None,
            driven: Default::default(),
            tracked: Tracked::default(),
            recovery: Recovery::default(),
        };
        world.build_bodies(scene);
        world.resync_joints(scene);
        world.sync_enabled(scene, &Owners::All);
        // Prime the query tree so a raycast works before the first step.
        let all: Vec<ColliderHandle> = world.colliders.iter().map(|(h, _)| h).collect();
        world.refresh_query_tree(&all);
        world
    }

    /// Push one entity's snapshot into its rapier body for this tick. A body
    /// that already holds the snapshot is not borrowed at all ([`body_to_write`]).
    pub(super) fn apply_body_state(&mut self, handle: RigidBodyHandle, snap: &EntityBodyState) {
        let class = body_type(snap);
        let pose = to_pose(snap.pos, snap.rot);
        let scale = gravity_scale(snap.use_gravity);
        let poses = &mut self.tracked.static_poses;
        if !snap.is_static {
            poses.remove(&handle);
        }
        let pushed = poses.get(&handle).copied();
        let differs = |b: &RigidBody| state_differs(b, snap, class, pose, scale, pushed.as_ref());
        let Some(body) = body_to_write(&mut self.bodies, handle, differs) else {
            return;
        };
        if body.body_type() != class {
            // A Rigidbody switched class mid-play (`SetKinematic`, a ragdoll
            // handoff, #466): the body changes in place at the entity's current
            // pose, so its colliders, joints and contacts survive.
            body.set_body_type(class, true);
            body.set_position(pose, true);
        }
        // Write the CCD mode whenever it differs, so `Physics.SetCollisionDetection`
        // toggled mid-play takes effect (mirrors the `gravity_scale` write).
        body.enable_ccd(snap.ccd_enabled);
        if snap.kinematic {
            // A pure mover (Unity's kinematic Rigidbody): it goes exactly where
            // its Transform says, through anything in the way.
            body.set_next_kinematic_position(pose);
        } else if snap.is_static {
            body.set_position(pose, true);
            poses.insert(handle, pose);
        } else {
            // Dynamic: trust rapier for pose, but let scripts inject linear and
            // angular velocity (SetVelocity / SetAngularVelocity / AddForce mutate
            // the component between ticks) and re-apply `use_gravity` so toggling
            // it at runtime takes effect.
            body.set_linvel(snap.vel, true);
            body.set_angvel(snap.angular_velocity, true);
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
    /// A zero-length step before the first real one skips rapier (#1795), and a
    /// panic inside rapier rebuilds the world instead, in a release build (#1797);
    /// both are `recover`'s.
    pub fn step(&mut self, scene: &mut Scene, dt: f32) -> PhysicsEvents {
        self.integration_parameters.dt = dt;
        let synced = self.sync_to_rapier(scene);
        if self.dormant(dt) {
            return self.dormant_step(scene, &synced);
        }
        self.drive_joints(scene);
        let awake_before = self.awake_owners();
        let pre_solve = self.snapshot_velocities();

        if let Err(message) = self.run_pipeline() {
            return self.recover(scene, message);
        }
        self.recovery.bootstrapped |= dt > 0.0;
        self.recovery.panics_in_a_row = 0;
        // Every body the step may have changed: rapier neither moves a sleeping
        // body nor wakes one without leaving it awake. The next sync visits them
        // too, named or not (`Tracked::stepped`).
        let mut stepped = awake_before;
        stepped.extend(self.awake_owners());
        self.tracked.stepped.clone_from(&stepped);

        let overlaps = self.overlap_pairs();
        let triggers = TriggerEvents::from_overlap_sets(&self.prev_triggers, overlaps);
        self.prev_triggers = triggers.stayed.clone();
        let contacts = self.collect_contact_pairs(&pre_solve);
        let collisions = CollisionEvents::from_contact_sets(&self.prev_collisions, contacts);
        self.prev_collisions = collisions.stayed_keys();
        let joint_breaks = self.break_joints(scene, &pre_solve, dt);
        self.sync_from_rapier(scene, &synced, stepped);
        PhysicsEvents {
            triggers,
            collisions,
            joint_breaks,
            rebuilt: None,
        }
    }

    /// One step of rapier's pipeline over the whole world.
    pub(super) fn pipeline_step(&mut self) {
        #[cfg(test)]
        if std::mem::take(&mut self.recovery.panic_next) {
            panic!("a panic forced through the test seam");
        }
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
    }

    /// Write one owner's integrated pose and velocities back onto its entity;
    /// whether its local Transform took a new value (its subtree moved).
    pub(super) fn write_back(&self, scene: &mut Scene, owner: u32) -> bool {
        let body = self
            .id_to_body
            .get(&owner)
            .and_then(|&h| self.bodies.get(h));
        let Some(body) = body.filter(|_| self.plan.contains_key(&owner)) else {
            return false;
        };
        // Only a dynamic body's pose is the solver's. A kinematic one ended the
        // step where its Transform put it, so writing it back would only add
        // world↔local rounding to every animated bone each tick.
        let mut moved = false;
        if body.is_dynamic() {
            let (pos, rot) = from_pose(body.position());
            let (local_pos, local_rot) = world_to_local(scene, owner, pos, rot);
            let held = scene
                .world
                .transform(owner)
                .map(|t| (t.position, t.rotation));
            moved = held.is_some_and(|held| held != (local_pos, local_rot));
            if let Some(mut t) = scene.world.transform_mut(owner) {
                t.position = local_pos;
                t.rotation = local_rot;
            }
        }
        // Compared before borrowing: a borrow stamps the Rigidbody as edited, and a
        // body at rest must not name itself to the next tick's sync (#1707).
        let (vel, ang) = (body.linvel(), body.angvel());
        let new_velocity = scene.world.rigidbody(owner).is_some_and(|rb| {
            !rb.is_kinematic && (rb.velocity != vel || rb.angular_velocity != ang)
        });
        if new_velocity {
            if let Some(mut rb) = scene.world.rigidbody_mut(owner) {
                rb.velocity = vel;
                rb.angular_velocity = ang;
            }
        }
        moved
    }
}
