//! crates/physics/src/differs_tests.rs — `state_differs` and `material_differs` are
//! hand-kept copies of the comparisons their setters make (#1798). A field one
//! of them forgets is a write that silently never happens, so each field alone
//! must flip it.

use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use super::convert::to_pose;
use super::live::body_type;
use super::material::{combine_rule, material_differs};
use super::shape::build::{gravity_scale, EntityBodyState};
use super::world::state_differs;
use super::writes::Damping;
use rusty_components::{CombineMode, PhysicsMaterial};

/// A dynamic snapshot, then `edit` applied to it.
fn snap(edit: impl FnOnce(&mut EntityBodyState)) -> EntityBodyState {
    let mut s = EntityBodyState {
        pos: Vec3::new(1.0, 2.0, 3.0),
        rot: Quat::from_rotation_y(0.5),
        vel: Vec3::X,
        angular_velocity: Vec3::Y,
        kinematic: false,
        is_static: false,
        use_gravity: true,
        ccd_enabled: false,
        damping: Damping {
            linear: 0.0,
            angular: 0.05,
        },
    };
    edit(&mut s);
    s
}

/// A body holding exactly what `s` says.
fn holding(s: &EntityBodyState) -> RigidBody {
    RigidBodyBuilder::new(body_type(s))
        .pose(to_pose(s.pos, s.rot))
        .linvel(s.vel)
        .angvel(s.angular_velocity)
        .gravity_scale(gravity_scale(s.use_gravity))
        .ccd_enabled(s.ccd_enabled)
        .linear_damping(s.damping.linear)
        .angular_damping(s.damping.angular)
        .build()
}

fn differs(b: &RigidBody, s: &EntityBodyState) -> bool {
    let pose = to_pose(s.pos, s.rot);
    state_differs(b, s, body_type(s), pose, gravity_scale(s.use_gravity), None)
}

#[test]
fn each_body_field_alone_flips_state_differs() {
    let kinematic = |s: &mut EntityBodyState| s.kinematic = true;
    for base in [snap(|_| {}), snap(kinematic)] {
        assert!(
            !differs(&holding(&base), &base),
            "a body holding its snapshot"
        );
    }
    let dynamic: [fn(&mut EntityBodyState); 8] = [
        |s| s.damping.linear = 1.0,
        |s| s.damping.angular = 0.0,
        |s| s.vel = Vec3::Z,
        |s| s.angular_velocity = Vec3::Z,
        |s| s.use_gravity = false,
        |s| s.ccd_enabled = true,
        |s| s.kinematic = true,
        |s| s.is_static = true,
    ];
    let held = holding(&snap(|_| {}));
    for (n, edit) in dynamic.into_iter().enumerate() {
        assert!(differs(&held, &snap(edit)), "dynamic field {n}");
    }
    let held = holding(&snap(kinematic));
    let moved: [fn(&mut EntityBodyState); 2] = [|s| s.pos = Vec3::ZERO, |s| s.rot = Quat::IDENTITY];
    for (n, edit) in moved.into_iter().enumerate() {
        assert!(
            differs(
                &held,
                &snap(|s| {
                    kinematic(s);
                    edit(s);
                })
            ),
            "pose field {n}"
        );
    }
}

/// rapier copies `next_position` into `position` only for an active body, so a
/// disabled kinematic body that was moved reads as placed once `next` is.
#[test]
fn a_disabled_body_is_placed_by_its_next_pose_alone() {
    let s = snap(|s| s.kinematic = true);
    let mut b = holding(&snap(|s| (s.kinematic, s.pos) = (true, Vec3::ZERO)));
    b.set_next_kinematic_position(to_pose(s.pos, s.rot));
    assert!(differs(&b, &s), "an active body whose position lags");
    b.set_enabled(false);
    assert!(!differs(&b, &s), "a disabled one");
}

#[test]
fn each_material_field_alone_flips_material_differs() {
    let base = PhysicsMaterial::default();
    let held = ColliderBuilder::ball(0.5)
        .friction(base.friction)
        .restitution(base.bounciness)
        .friction_combine_rule(combine_rule(base.friction_combine))
        .restitution_combine_rule(combine_rule(base.bounce_combine))
        .build();
    assert!(!material_differs(&held, &base));
    let edits: [fn(&mut PhysicsMaterial); 4] = [
        |m| m.friction += 0.25,
        |m| m.bounciness += 0.25,
        |m| m.friction_combine = CombineMode::Maximum,
        |m| m.bounce_combine = CombineMode::Maximum,
    ];
    for (n, edit) in edits.into_iter().enumerate() {
        let mut m = base;
        edit(&mut m);
        assert!(material_differs(&held, &m), "material field {n}");
    }
}
