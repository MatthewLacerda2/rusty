//! src/editor/inspector/components/gameplay/physics.rs — the physics-flavoured
//! gameplay inspector cards: RigidBody and NavMesh Agent. The Collider card lives in
//! the sibling `collider` module; all three were split out of the gameplay card
//! module to stay under the size cap.
//!
//! Each is a THIN client (#287): widgets read fields from a snapshot and route every
//! write through a shared `authoring::*` op, never a mutable component borrow; the
//! `Physics.*` / `NavMeshAgent.*` Lua setters call the same ops, so the panels and the
//! bindings share one write.

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::{nav_agent as nav_ops, rigidbody as rigidbody_ops};
use crate::scene::{CollisionDetection, Entity};

/// 3EG. RigidBody Component
pub fn draw_rigidbody(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let Some(rb) = entity.rigidbody.clone() else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::CUBE, "RigidBody", Some(&mut remove), |ui| {
        let mut active = rb.active;
        if ui.checkbox(&mut active, "Active").changed() {
            rigidbody_ops::set_active(entity, active);
            *is_dirty = true;
        }
        let mut is_kinematic = rb.is_kinematic;
        if ui.checkbox(&mut is_kinematic, "Is Kinematic").changed() {
            rigidbody_ops::set_kinematic(entity, is_kinematic);
            *is_dirty = true;
        }
        let mut use_gravity = rb.use_gravity;
        if ui.checkbox(&mut use_gravity, "Use Gravity").changed() {
            rigidbody_ops::set_use_gravity(entity, use_gravity);
            *is_dirty = true;
        }
        let mut mass = rb.mass;
        ui.horizontal(|ui| {
            ui.label("Mass:");
            if ui
                .add(
                    egui::DragValue::new(&mut mass)
                        .speed(0.05)
                        .clamp_range(0.01..=1000.0),
                )
                .changed()
            {
                rigidbody_ops::set_mass(entity, mass);
                *is_dirty = true;
            }
        });
        draw_rb_velocity(ui, entity, rb.velocity, is_dirty);
        draw_rb_angular_velocity(ui, entity, rb.angular_velocity, is_dirty);
        draw_rb_collision_detection(ui, entity, rb.collision_detection, is_dirty);
    });
    if remove {
        entity.rigidbody = None;
        *is_dirty = true;
    }
}

/// The rigidbody velocity x/y/z row, routing a change through the shared op.
fn draw_rb_velocity(ui: &mut egui::Ui, entity: &mut Entity, velocity: Vec3, is_dirty: &mut bool) {
    if let Some(v) = vec3_row(ui, "Velocity:", velocity) {
        rigidbody_ops::set_velocity(entity, v);
        *is_dirty = true;
    }
}

/// The rigidbody angular-velocity x/y/z row (radians/sec per axis), routing a
/// change through the shared op.
fn draw_rb_angular_velocity(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    angular_velocity: Vec3,
    is_dirty: &mut bool,
) {
    if let Some(w) = vec3_row(ui, "Angular Velocity:", angular_velocity) {
        rigidbody_ops::set_angular_velocity(entity, w);
        *is_dirty = true;
    }
}

/// The Discrete/Continuous collision-detection combo, routing a change through
/// the shared op (editor↔API parity with `Physics.SetCollisionDetection`).
fn draw_rb_collision_detection(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    current: CollisionDetection,
    is_dirty: &mut bool,
) {
    let mut mode = current;
    egui::ComboBox::from_label("Collision Detection")
        .selected_text(mode.as_str())
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut mode, CollisionDetection::Discrete, "Discrete");
            ui.selectable_value(&mut mode, CollisionDetection::Continuous, "Continuous");
        });
    if mode != current {
        rigidbody_ops::set_collision_detection(entity, mode);
        *is_dirty = true;
    }
}

/// 3EH. NavMeshAgent Component
pub fn draw_nav_agent(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let Some(agent) = entity.nav_agent.clone() else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::PATH, "NavMesh Agent", Some(&mut remove), |ui| {
        let mut active = agent.active;
        if ui.checkbox(&mut active, "Active").changed() {
            nav_ops::set_active(entity, active);
            *is_dirty = true;
        }
        let mut speed = agent.speed;
        if clamped(ui, "Speed:", &mut speed, 0.0..=100.0) {
            nav_ops::set_speed(entity, speed);
            *is_dirty = true;
        }
        let mut acceleration = agent.acceleration;
        if clamped(ui, "Acceleration:", &mut acceleration, 0.0..=100.0) {
            nav_ops::set_acceleration(entity, acceleration);
            *is_dirty = true;
        }
        let mut stopping_distance = agent.stopping_distance;
        if clamped(ui, "Stopping Distance:", &mut stopping_distance, 0.0..=50.0) {
            nav_ops::set_stopping_distance(entity, stopping_distance);
            *is_dirty = true;
        }
        let mut radius = agent.radius;
        if clamped(ui, "Radius:", &mut radius, 0.01..=10.0) {
            nav_ops::set_radius(entity, radius);
            *is_dirty = true;
        }
        draw_agent_target(ui, entity, agent.target, is_dirty);
        ui.label(format!(
            "Velocity: [{:.2}, {:.2}, {:.2}]",
            agent.velocity.x, agent.velocity.y, agent.velocity.z
        ));
    });
    if remove {
        entity.nav_agent = None;
        *is_dirty = true;
    }
}

/// The nav-agent destination x/y/z row, routing a change through the shared op.
fn draw_agent_target(ui: &mut egui::Ui, entity: &mut Entity, target: Vec3, is_dirty: &mut bool) {
    if let Some(t) = vec3_row(ui, "Target:", target) {
        nav_ops::set_target(entity, t);
        *is_dirty = true;
    }
}

/// A labelled x/y/z drag row over a `Vec3`. Returns the new value when any axis
/// changed (so the caller routes it through the matching shared op), else `None`.
fn vec3_row(ui: &mut egui::Ui, label: &str, value: Vec3) -> Option<Vec3> {
    let mut value = value;
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            let mut c = ui
                .add(egui::DragValue::new(&mut value.x).speed(0.1))
                .changed();
            c |= ui
                .add(egui::DragValue::new(&mut value.y).speed(0.1))
                .changed();
            c |= ui
                .add(egui::DragValue::new(&mut value.z).speed(0.1))
                .changed();
            c
        })
        .inner;
    changed.then_some(value)
}

/// A labelled, clamped drag row (shared by the nav-agent scalar fields). Returns
/// whether the value changed.
fn clamped(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.05).clamp_range(range))
            .changed()
    })
    .inner
}
