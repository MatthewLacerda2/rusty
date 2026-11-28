use rusty_core::collections::Map;

use rapier3d::prelude::*;

use super::character::ControllerBodies;
use super::collider_ids::ColliderIds;
use super::compound::{world_to_local, BodyPlan};
use super::convert::{from_pose, to_pose};
use super::events::collision::CollisionEvents;
use super::events::trigger::TriggerEvents;
use super::joints::JointMap;
use super::live::body_type;
use super::recover::Recovery;
use super::shape::build::{body_state, gravity_scale, EntityBodyState};
use super::watch::{Owners, Tracked};
pub(super) use super::writes::{body_to_write, collider_to_write, state_differs};
use super::PhysicsEvents;
use rusty_scene::Scene;

pub struct PhysicsWorld {
    pub(super) gravity: Vector,
    pub(super) integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    pub(super) islands: IslandManager,
    pub(super) broad_phase: DefaultBroadPhase,
    pub(super) narrow_phase: NarrowPhase,
    pub(super) bodies: RigidBodySet,
    pub(super) colliders: ColliderSet,
    pub(super) impulse_joints: ImpulseJointSet,
    pub(super) multibody_joints: MultibodyJointSet,
    pub(super) soft_bodies: SoftBodySet,
    ccd_solver: CCDSolver,
    pub(super) plan: BodyPlan,
    pub(super) id_to_body: Map<u32, RigidBodyHandle>,
    pub(super) body_to_id: Map<RigidBodyHandle, u32>,
    pub(super) collider_to_id: Map<ColliderHandle, u32>,
    pub(super) id_to_collider: ColliderIds,
    /// Empty at first, so a session's first overlap is an enter.
    pub(super) prev_triggers: Vec<(u32, u32)>,
    pub(super) prev_collisions: Vec<(u32, u32)>,
    pub(super) joints: JointMap,
    pub(super) world_body: Option<RigidBodyHandle>,
    pub(super) driven: std::collections::BTreeMap<u32, super::joints::drive::Driven>,
    pub(super) tracked: Tracked,
    pub(super) recovery: Recovery,
    pub(super) controller_bodies: ControllerBodies,
}

impl PhysicsWorld {
    pub fn from_scene(scene: &Scene) -> Self {
        let mut world = Self {
            gravity: Vector::new(0.0, -9.81, 0.0),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: super::contact::narrow_phase(),
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
            id_to_collider: ColliderIds::default(),
            prev_triggers: Vec::new(),
            prev_collisions: Vec::new(),
            joints: JointMap::new(),
            world_body: None,
            driven: Default::default(),
            tracked: Tracked::default(),
            recovery: Recovery::default(),
            controller_bodies: ControllerBodies::default(),
        };
        world.build_bodies(scene);
        world.resync_joints(scene);
        world.sync_enabled(scene, &Owners::All);
        // Primed: a raycast works before the first step.
        let all: Vec<ColliderHandle> = world.colliders.iter().map(|(h, _)| h).collect();
        world.refresh_query_tree(&all);
        world
    }

    pub(super) fn warp_bodies(&mut self, scene: &Scene, cut: &[u32]) {
        for id in cut {
            let Some(&handle) = self.id_to_body.get(id) else {
                continue;
            };
            let (Some(snap), Some(body)) = (body_state(scene, *id), self.bodies.get_mut(handle))
            else {
                continue;
            };
            if body.is_dynamic() {
                body.set_position(to_pose(snap.pos, snap.rot), true);
            }
        }
    }

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
            // In place: colliders, joints and contacts survive.
            self.controller_bodies.retype(body, class);
            body.set_position(pose, true);
        }
        body.enable_ccd(snap.ccd_enabled);
        if snap.kinematic {
            // Unity's kinematic Rigidbody: it goes through anything.
            body.set_next_kinematic_position(pose);
        } else if snap.is_static {
            body.set_position(pose, true);
            poses.insert(handle, pose);
        } else {
            body.set_linvel(snap.vel, true);
            body.set_angvel(snap.angular_velocity, true);
            if !snap.damping.held_by(body) {
                // No wake: damping only scales velocity.
                body.set_linear_damping(snap.damping.linear);
                body.set_angular_damping(snap.damping.angular);
            }
            if body.gravity_scale() != scale {
                // A full wake: rapier's setter only clears the sleep flag, so a
                // weightless sleeper would doze off again.
                body.set_gravity_scale(scale, false);
                body.wake_up(true);
            }
        }
    }

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
        // Awake before or after: rapier moves no body asleep on both sides.
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

    pub(super) fn pipeline_step(&mut self) {
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

    pub(super) fn write_back(&self, scene: &mut Scene, owner: u32) -> bool {
        let body = self
            .id_to_body
            .get(&owner)
            .and_then(|&h| self.bodies.get(h));
        let Some(body) = body.filter(|_| self.plan.contains_key(&owner)) else {
            return false;
        };
        // Only a dynamic pose is the solver's; writing a kinematic one back adds world↔local
        // rounding to every animated bone each tick.
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
        // Compared before borrowing: a borrow stamps the Rigidbody as edited.
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
