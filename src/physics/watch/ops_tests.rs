//! src/physics/watch/ops_tests.rs — the physics edits the incremental sync's
//! equivalence proptest runs (#1707, `tests.rs`), on top of the World's own
//! shared edit sequence (`ecs::changes::ops_tests`).

use glam::Vec3;
use proptest::prelude::*;

use super::fixture_tests::world_op;
use crate::components::{
    CharacterControllerComponent, ColliderComponent, ColliderShape, JointComponent,
    RigidBodyComponent,
};
use crate::ecs::changes::ops_tests;
use crate::physics::PhysicsWorld;
use crate::scene::authoring::defaults::{default_collider, default_rigidbody};
use crate::scene::Scene;

#[derive(Clone, Debug)]
pub enum Phys {
    Collider(u8),
    NoCollider,
    ColliderActive(bool),
    Trigger(bool),
    Friction(u8),
    Body(bool),
    NoBody,
    Velocity(i8),
    Mass(u8),
    Character(u8),
    NoCharacter,
    Joint(Option<usize>),
    NoJoint,
}

#[derive(Clone, Debug)]
pub enum Op {
    World(ops_tests::Op),
    On(usize, Phys),
    Matrix(u8, bool),
    Step,
    /// A step at `Time.timeScale` 0: `dt` is 0.
    Pause,
}

fn phys() -> impl Strategy<Value = Phys> {
    let flag = any::<bool>;
    prop_oneof![
        3 => (0u8..3).prop_map(Phys::Collider),
        1 => Just(Phys::NoCollider),
        1 => flag().prop_map(Phys::ColliderActive),
        1 => flag().prop_map(Phys::Trigger),
        1 => (0u8..3).prop_map(Phys::Friction),
        3 => flag().prop_map(Phys::Body),
        1 => Just(Phys::NoBody),
        2 => any::<i8>().prop_map(Phys::Velocity),
        1 => (1u8..4).prop_map(Phys::Mass),
        1 => (1u8..4).prop_map(Phys::Character),
        1 => Just(Phys::NoCharacter),
        1 => proptest::option::of(0usize..10).prop_map(Phys::Joint),
        1 => Just(Phys::NoJoint),
    ]
}

pub fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => ops_tests::op().prop_map(Op::World),
        8 => (0usize..10, phys()).prop_map(|(i, p)| Op::On(i, p)),
        1 => (0u8..3, any::<bool>()).prop_map(|(l, on)| Op::Matrix(l, on)),
        6 => Just(Op::Step),
        1 => Just(Op::Pause),
    ]
}

fn shape(kind: u8) -> ColliderShape {
    match kind {
        0 => ColliderShape::Box { size: Vec3::ONE },
        1 => ColliderShape::Sphere { radius: 0.5 },
        _ => ColliderShape::Box {
            size: Vec3::new(4.0, 0.5, 4.0),
        },
    }
}

/// Apply `op` to `scene`; a step steps `physics`.
pub fn apply(scene: &mut Scene, physics: &mut PhysicsWorld, op: &Op) {
    let (e, edit) = match op {
        Op::World(op) => return world_op(&mut scene.world, op),
        Op::Matrix(l, on) => return scene.collision_matrix.set_collision(0, *l, *on),
        Op::Step | Op::Pause => {
            let dt = if matches!(op, Op::Step) {
                1.0 / 60.0
            } else {
                0.0
            };
            physics.step(scene, dt);
            return;
        }
        Op::On(i, edit) => match scene.world.ids().get(*i).copied() {
            Some(e) => (e, edit),
            None => return,
        },
    };
    let w = &mut scene.world;
    let pick = |w: &crate::ecs::World, i: usize| w.ids().get(i).copied();
    match *edit {
        Phys::Collider(k) => {
            let c = ColliderComponent {
                shape: shape(k),
                ..default_collider()
            };
            w.set_collider(e, Some(c));
        }
        Phys::NoCollider => drop(w.set_collider(e, None)),
        Phys::ColliderActive(on) => drop(w.collider_mut(e).map(|mut c| c.active = on)),
        Phys::Trigger(on) => drop(w.collider_mut(e).map(|mut c| c.is_trigger = on)),
        Phys::Friction(f) => drop(
            w.collider_mut(e)
                .map(|mut c| c.material.friction = f32::from(f) * 0.4),
        ),
        Phys::Body(kinematic) => {
            let rb = RigidBodyComponent {
                is_kinematic: kinematic,
                ..default_rigidbody()
            };
            w.set_rigidbody(e, Some(rb));
        }
        Phys::NoBody => drop(w.set_rigidbody(e, None)),
        Phys::Velocity(v) => {
            let v = Vec3::new(f32::from(v) * 0.05, 0.0, 0.0);
            let _ = w.rigidbody_mut(e).map(|mut rb| rb.velocity = v);
        }
        Phys::Mass(m) => drop(w.rigidbody_mut(e).map(|mut rb| rb.mass = f32::from(m))),
        Phys::Character(h) => {
            let cc = CharacterControllerComponent {
                height: f32::from(h),
                ..Default::default()
            };
            w.set_character_controller(e, Some(cc));
        }
        Phys::NoCharacter => drop(w.set_character_controller(e, None)),
        Phys::Joint(other) => {
            let j = JointComponent {
                connected_body: other.and_then(|i| pick(w, i)),
                ..Default::default()
            };
            w.set_joint(e, Some(j));
        }
        Phys::NoJoint => drop(w.set_joint(e, None)),
    }
}
