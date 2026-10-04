//! src/editor/inspector/components/gameplay/physics.rs — the physics-flavoured
//! gameplay inspector card: RigidBody. The Collider and NavMesh Agent cards live in
//! the sibling `collider` and `nav_agent` modules; all three were split out of the
//! gameplay card module to stay under the size cap.
//!
//! Each is a THIN client (#287): widgets read fields from a snapshot and route every
//! write through a shared `authoring::*` op, never a mutable component borrow; the
//! `Physics.*` Lua setters call the same ops, so the panels and the
//! bindings share one write.

use egui_phosphor::regular as icon;
use glam::Vec3;

use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rigidbody as rigidbody_ops;
use crate::scene::CollisionDetection;

/// A checkbox row that routes its write through a shared rigidbody op (#287).
#[allow(clippy::too_many_arguments)] // (world, id) replaced the single &mut Entity handle
fn rb_checkbox(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    label: &str,
    mut value: bool,
    op: fn(&mut crate::scene::RigidBodyComponent, bool),
    is_dirty: &mut bool,
) {
    if ui.checkbox(&mut value, label).changed() {
        if let Some(mut r) = world.rigidbody_mut(id) {
            op(&mut r, value);
        }
        *is_dirty = true;
    }
}

/// The Mass drag row (clamped ≥ 0.01), routed through the shared op (#287).
fn draw_rb_mass(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    mut mass: f32,
    is_dirty: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label("Mass:");
        let drag = egui::DragValue::new(&mut mass)
            .speed(0.05)
            .range(0.01..=1000.0);
        if ui.add(drag).changed() {
            if let Some(mut r) = world.rigidbody_mut(id) {
                rigidbody_ops::set_mass(&mut r, mass);
            }
            *is_dirty = true;
        }
    });
}

/// 3EG. RigidBody Component
pub fn draw_rigidbody(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(rb) = world.rigidbody(id).map(|rb| rb.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::CUBE, "RigidBody", Some(&mut remove), |ui| {
        rb_checkbox(
            ui,
            world,
            id,
            "Active",
            rb.active,
            rigidbody_ops::set_active,
            is_dirty,
        );
        rb_checkbox(
            ui,
            world,
            id,
            "Is Kinematic",
            rb.is_kinematic,
            rigidbody_ops::set_kinematic,
            is_dirty,
        );
        rb_checkbox(
            ui,
            world,
            id,
            "Use Gravity",
            rb.use_gravity,
            rigidbody_ops::set_use_gravity,
            is_dirty,
        );
        draw_rb_mass(ui, world, id, rb.mass, is_dirty);
        draw_rb_velocity(ui, world, id, rb.velocity, is_dirty);
        draw_rb_angular_velocity(ui, world, id, rb.angular_velocity, is_dirty);
        draw_rb_collision_detection(ui, world, id, rb.collision_detection, is_dirty);
    });
    if remove {
        world.set_rigidbody(id, None);
        *is_dirty = true;
    }
}

/// The rigidbody velocity x/y/z row, routing a change through the shared op.
fn draw_rb_velocity(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    velocity: Vec3,
    is_dirty: &mut bool,
) {
    if let Some(v) = vec3_row(ui, "Velocity:", velocity) {
        if let Some(mut r) = world.rigidbody_mut(id) {
            rigidbody_ops::set_velocity(&mut r, v);
        }
        *is_dirty = true;
    }
}

/// The rigidbody angular-velocity x/y/z row (radians/sec per axis), routing a
/// change through the shared op.
fn draw_rb_angular_velocity(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    angular_velocity: Vec3,
    is_dirty: &mut bool,
) {
    if let Some(w) = vec3_row(ui, "Angular Velocity:", angular_velocity) {
        if let Some(mut r) = world.rigidbody_mut(id) {
            rigidbody_ops::set_angular_velocity(&mut r, w);
        }
        *is_dirty = true;
    }
}

/// The Discrete/Continuous collision-detection combo, routing a change through
/// the shared op (editor↔API parity with `Physics.SetCollisionDetection`).
fn draw_rb_collision_detection(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
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
        if let Some(mut r) = world.rigidbody_mut(id) {
            rigidbody_ops::set_collision_detection(&mut r, mode);
        }
        *is_dirty = true;
    }
}

/// A labelled x/y/z drag row over a `Vec3`. Returns the new value when any axis
/// changed (so the caller routes it through the matching shared op), else `None`.
pub(super) fn vec3_row(ui: &mut egui::Ui, label: &str, value: Vec3) -> Option<Vec3> {
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
