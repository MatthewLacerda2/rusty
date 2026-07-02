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

use super::build::{
    body_state, build_shape, collider_inputs, gravity_scale, interaction_groups, order_pair,
    BodyClass, EntityBodyState,
};
use super::character;
use super::convert::{from_iso, from_na_vec, to_iso, to_na_vec};
use super::trigger_events::TriggerEvents;
use crate::scene::Scene;

pub struct PhysicsWorld {
    gravity: Vector<Real>,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    pub(super) bodies: RigidBodySet,
    pub(super) colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,
    /// Exposed to the `physics` module (see `query`) for ray casts.
    pub(super) query_pipeline: QueryPipeline,
    /// Per-body downward fall speed for gravity-driven kinematic bodies (#318),
    /// keyed by entity id and carried across ticks; zeroed on ground contact.
    fall_speeds: HashMap<u32, f32>,
    /// stable entity id -> rigid-body handle.
    id_to_body: HashMap<u32, RigidBodyHandle>,
    /// collider handle -> stable entity id (for event/raycast lookup).
    pub(super) collider_to_id: HashMap<ColliderHandle, u32>,
    /// stable entity id -> its trigger flag (sensors surface trigger pairs).
    id_is_trigger: HashMap<u32, bool>,
    /// Last tick's trigger-overlap pairs, diffed each step to recover the
    /// enter/exit edges (#310). Starts empty, so a play session's first
    /// overlapping tick is an "enter".
    prev_triggers: Vec<(u32, u32)>,
}

impl PhysicsWorld {
    /// Build the rapier world from the current scene. One rigid body + one
    /// collider per entity that has a `ColliderComponent`.
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
            id_to_body: HashMap::new(),
            collider_to_id: HashMap::new(),
            id_is_trigger: HashMap::new(),
            prev_triggers: Vec::new(),
        };
        world.build_bodies(scene);
        // Prime the query pipeline so a raycast works before the first step.
        world.query_pipeline.update(&world.bodies, &world.colliders);
        world
    }

    fn build_bodies(&mut self, scene: &Scene) {
        for id in scene.entity_ids() {
            let Some(inp) = scene.get_entity(id).as_deref().and_then(collider_inputs) else {
                continue;
            };

            // Build the collider first: a mesh collider with missing/degenerate
            // geometry yields `None`, in which case the entity gets no body at all.
            let mesh_ref = inp
                .mesh_geom
                .as_ref()
                .map(|(p, i)| (p.as_slice(), i.as_slice()));
            let Some(mut collider) = build_shape(&inp.shape, inp.scale, mesh_ref) else {
                continue;
            };

            let body_builder = match inp.class {
                BodyClass::Static => RigidBodyBuilder::fixed(),
                BodyClass::Kinematic => RigidBodyBuilder::kinematic_position_based(),
                BodyClass::Dynamic => RigidBodyBuilder::dynamic()
                    .linvel(to_na_vec(inp.velocity))
                    // `use_gravity = false` exempts the body from world gravity (#209).
                    .gravity_scale(gravity_scale(inp.use_gravity)),
            }
            .position(to_iso(inp.pos, inp.rot));
            let body_handle = self.bodies.insert(body_builder.build());
            collider.set_sensor(inp.is_trigger);
            collider.set_active_events(ActiveEvents::COLLISION_EVENTS);
            // The demo's bodies are kinematic/static, so the default
            // "only-if-one-is-dynamic" filtering would suppress every player↔wall
            // and player↔enemy pair. Enable all type combinations.
            collider.set_active_collision_types(ActiveCollisionTypes::all());
            // The collision matrix (#91) decides which layers interact: a collider
            // is a member of its own layer and filters to the layers it may collide
            // with. Set on both collision and solver groups so contact generation
            // and the solver agree.
            let groups =
                interaction_groups(inp.layer, scene.collision_matrix.filter_mask(inp.layer));
            collider.set_collision_groups(groups);
            collider.set_solver_groups(groups);
            let collider_handle =
                self.colliders
                    .insert_with_parent(collider, body_handle, &mut self.bodies);
            self.id_to_body.insert(id, body_handle);
            self.collider_to_id.insert(collider_handle, id);
            self.id_is_trigger
                .insert(id, inp.is_trigger || inp.is_static);
        }
    }

    /// Push current component state into rapier before stepping.
    fn sync_to_rapier(&mut self, scene: &Scene, dt: f32) {
        // Snapshot handles to avoid borrowing the map while the body sets below
        // borrow `self.bodies` / `self.colliders` mutably or immutably.
        let entries: Vec<(u32, RigidBodyHandle)> =
            self.id_to_body.iter().map(|(&id, &h)| (id, h)).collect();
        for (id, handle) in entries {
            let Some(snapshot) = scene.get_entity(id).as_deref().map(body_state) else {
                continue;
            };
            if self.bodies.get(handle).is_none() {
                continue;
            }
            self.apply_body_state(id, handle, &snapshot, dt);
        }
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
            body.set_next_kinematic_position(next);
            return;
        }
        // Leaving the kinematic class (`Physics.SetKinematic`) drops any carried
        // fall speed, so toggling back later starts a fresh fall.
        self.fall_speeds.remove(&id);
        let body = &mut self.bodies[handle];
        body.set_enabled(snap.active);
        if snap.is_static {
            body.set_position(to_iso(snap.pos, snap.rot), true);
        } else {
            // Dynamic: trust rapier for pose, but let scripts inject velocity
            // (SetVelocity / AddForce mutate the component between ticks) and
            // re-apply `use_gravity` so toggling it at runtime takes effect.
            body.set_linvel(to_na_vec(snap.vel), true);
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

        let events = TriggerEvents::from_overlap_sets(&self.prev_triggers, self.collect_triggers());
        self.prev_triggers = events.stayed.clone();
        self.sync_from_rapier(scene);
        events
    }

    /// Gather overlapping pairs that involve a trigger/sensor or static body,
    /// preserving the legacy `Vec<(u32, u32)>` trigger-pair contract. Sensors land
    /// in the intersection graph; solid contacts in the contact graph.
    fn collect_triggers(&self) -> Vec<(u32, u32)> {
        let mut pairs = Vec::new();
        for (h1, h2, intersecting) in self.narrow_phase.intersection_pairs() {
            if !intersecting {
                continue;
            }
            if let (Some(&a), Some(&b)) =
                (self.collider_to_id.get(&h1), self.collider_to_id.get(&h2))
            {
                pairs.push(order_pair(a, b));
            }
        }
        for contact in self.narrow_phase.contact_pairs() {
            if !contact.has_any_active_contact {
                continue;
            }
            let a = self.collider_to_id.get(&contact.collider1);
            let b = self.collider_to_id.get(&contact.collider2);
            if let (Some(&a), Some(&b)) = (a, b) {
                let trig_a = *self.id_is_trigger.get(&a).unwrap_or(&false);
                let trig_b = *self.id_is_trigger.get(&b).unwrap_or(&false);
                if trig_a || trig_b {
                    pairs.push(order_pair(a, b));
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        pairs
    }

    /// Write integrated poses + velocities back onto the entities.
    fn sync_from_rapier(&self, scene: &mut Scene) {
        for (&id, &handle) in &self.id_to_body {
            let (pos, rot, vel) = {
                let body = match self.bodies.get(handle) {
                    Some(b) => b,
                    None => continue,
                };
                let (pos, rot) = from_iso(body.position());
                (pos, rot, from_na_vec(*body.linvel()))
            };
            if let Some(mut entity) = scene.get_entity_mut(id) {
                entity.transform.position = pos;
                entity.transform.rotation = rot;
                if let Some(rb) = &mut entity.rigidbody {
                    if !rb.is_kinematic {
                        rb.velocity = vel;
                    }
                }
            }
            scene.update_entity_collider(id);
        }
    }
}
