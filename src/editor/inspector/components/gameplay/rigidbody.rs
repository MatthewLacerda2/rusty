//! src/editor/inspector/components/gameplay/rigidbody.rs — the RigidBody
//! inspector card. Split out of `physics` (Collider/NavMesh Agent) to stay under
//! the size cap.
//!
//! A THIN client (#287): widgets read fields from a snapshot and route every
//! write through the shared `authoring::rigidbody` ops, never a mutable component
//! borrow; the `Physics.*` Lua setters call the same ops, so the panel and the
//! bindings share one write.

use egui_phosphor::regular as icon;
use glam::Vec3;

use super::physics::vec3_row;
use crate::components::CollisionDetection;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::rigidbody as rigidbody_ops;
use crate::scene::Entity;

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
        draw_collision_detection(ui, entity, rb.collision_detection, is_dirty);
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

/// The Discrete/Continuous collision-detection selector (#321), mirroring the
/// Camera card's `Clear Flags` combo pattern. Continuous enables CCD so a
/// small/fast body sweeps its motion instead of only testing overlap at the
/// tick's final pose, at extra solver cost.
fn draw_collision_detection(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    mode: CollisionDetection,
    is_dirty: &mut bool,
) {
    let mut mode = mode;
    ui.horizontal(|ui| {
        ui.label("Collision Detection:");
        egui::ComboBox::from_id_source("collision_detection")
            .selected_text(match mode {
                CollisionDetection::Discrete => "Discrete",
                CollisionDetection::Continuous => "Continuous",
            })
            .show_ui(ui, |ui| {
                for (value, label) in [
                    (CollisionDetection::Discrete, "Discrete"),
                    (CollisionDetection::Continuous, "Continuous"),
                ] {
                    if ui.selectable_value(&mut mode, value, label).changed() {
                        rigidbody_ops::set_collision_detection(entity, mode);
                        *is_dirty = true;
                    }
                }
            });
    });
}
