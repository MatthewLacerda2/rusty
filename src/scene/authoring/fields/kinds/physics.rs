//! src/scene/authoring/fields/kinds/physics.rs — the physics components' field tables.

use glam::Vec2;

use super::super::table::{inspect, op, with};
use crate::components::{
    CharacterControllerComponent, ColliderComponent, JointComponent, RigidBodyComponent,
};
use crate::scene::authoring::{character_controller as cc, collider, joint, rigidbody as rb};

inspect! {
    RigidBodyComponent => rigidbody, rigidbody_mut, &[
        op!("active", rb::set_active),
        op!("is_kinematic", rb::set_kinematic),
        op!("mass", rb::set_mass),
        op!("velocity", rb::set_velocity),
        op!("angular_velocity", rb::set_angular_velocity),
        op!("use_gravity", rb::set_use_gravity),
        op!("collision_detection", rb::set_collision_detection),
    ];
    ColliderComponent => collider, collider_mut, &[
        op!("active", collider::set_active),
        op!("shape", collider::set_shape),
        op!("is_trigger", collider::set_trigger),
        op!("material", collider::set_material),
    ];
    JointComponent => joint, joint_mut, &[
        op!("kind", joint::set_kind),
        op!("connected_body", joint::set_connected_body),
        op!("anchor", joint::set_anchor),
        op!("connected_anchor", joint::set_connected_anchor),
        op!("auto_configure_connected_anchor", joint::set_auto_configure_connected_anchor),
        op!("axis", joint::set_axis),
        op!("swing_axis", joint::set_swing_axis),
        op!("use_limits", joint::set_use_limits),
        with!("limits", |j, v: Vec2| joint::set_limits(j, v.x, v.y)),
        op!("swing_limit", joint::set_swing_limit),
        op!("swing2_limit", joint::set_swing2_limit),
        op!("break_force", joint::set_break_force),
        op!("break_torque", joint::set_break_torque),
        op!("enable_collision", joint::set_enable_collision),
    ];
    CharacterControllerComponent => character_controller, character_controller_mut, &[
        op!("height", cc::set_height),
        op!("radius", cc::set_radius),
        op!("center", cc::set_center),
        op!("step_offset", cc::set_step_offset),
        op!("slope_limit", cc::set_slope_limit),
        op!("skin_width", cc::set_skin_width),
        op!("min_move_distance", cc::set_min_move_distance),
    ];
}
