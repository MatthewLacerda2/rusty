//! src/physics/watch/edit_tests.rs — the physics edits the incremental sync's
//! equivalence proptest makes to one entity (#1707, `tests.rs`): a Collider, a
//! Rigidbody, a CharacterController or a Joint attached, edited or detached.

use glam::Vec3;
use proptest::prelude::*;

use crate::components::{
    CharacterControllerComponent, ColliderComponent, ColliderShape, JointComponent,
    RigidBodyComponent,
};
use crate::ecs::World;
use crate::scene::authoring::defaults::{default_collider, default_rigidbody};

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

pub fn phys() -> impl Strategy<Value = Phys> {
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

fn shape(kind: u8) -> ColliderShape {
    match kind {
        0 => ColliderShape::Box { size: Vec3::ONE },
        1 => ColliderShape::Sphere { radius: 0.5 },
        _ => ColliderShape::Box {
            size: Vec3::new(4.0, 0.5, 4.0),
        },
    }
}

/// Apply one physics edit to entity `e`.
pub fn edit_one(w: &mut World, e: u32, edit: &Phys) {
    match *edit {
        Phys::Collider(k) => {
            let c = ColliderComponent {
                shape: shape(k),
                ..default_collider()
            };
            w.set_collider(e, Some(c))
        }
        Phys::NoCollider => w.set_collider(e, None),
        Phys::ColliderActive(on) => w.collider_mut(e).map(|mut c| c.active = on).is_some(),
        Phys::Trigger(on) => w.collider_mut(e).map(|mut c| c.is_trigger = on).is_some(),
        Phys::Friction(f) => {
            let friction = f32::from(f) * 0.4;
            w.collider_mut(e)
                .map(|mut c| c.material.friction = friction)
                .is_some()
        }
        Phys::Body(kinematic) => {
            let rb = RigidBodyComponent {
                is_kinematic: kinematic,
                ..default_rigidbody()
            };
            w.set_rigidbody(e, Some(rb))
        }
        Phys::NoBody => w.set_rigidbody(e, None),
        Phys::Velocity(v) => {
            let v = Vec3::new(f32::from(v) * 0.05, 0.0, 0.0);
            w.rigidbody_mut(e).map(|mut rb| rb.velocity = v).is_some()
        }
        Phys::Mass(m) => w
            .rigidbody_mut(e)
            .map(|mut rb| rb.mass = f32::from(m))
            .is_some(),
        Phys::Character(h) => {
            let cc = CharacterControllerComponent {
                height: f32::from(h),
                ..Default::default()
            };
            w.set_character_controller(e, Some(cc))
        }
        Phys::NoCharacter => w.set_character_controller(e, None),
        Phys::Joint(other) => {
            let j = JointComponent {
                connected_body: other.and_then(|i| w.ids().get(i).copied()),
                ..Default::default()
            };
            w.set_joint(e, Some(j))
        }
        Phys::NoJoint => w.set_joint(e, None),
    };
}
