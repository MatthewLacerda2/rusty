use egui_phosphor::regular as icon;
use glam::Vec3;
use std::path::Path;

use crate::editor::inspector_card::component_card;
use crate::editor::theme;
use crate::scene::{ColliderShape, Entity};

/// 3D. Script bindings
pub fn draw_script(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.script.is_none() {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::SCROLL, "Lua Script", Some(&mut remove), |ui| {
        let Some(script) = &mut entity.script else {
            return;
        };
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut script.path);
        });
        let t = theme::from_ui(ui);
        if Path::new(&script.path).exists() {
            ui.colored_label(t.accent_blue, "✔ Loaded and ready to run");
        } else {
            ui.colored_label(t.danger, "❌ File not found!");
        }
    });
    if remove {
        entity.script = None;
        *is_dirty = true;
    }
}

/// 3E. Health component
pub fn draw_health(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.health.is_none() {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::HEART, "Health", Some(&mut remove), |ui| {
        let Some(health) = &mut entity.health else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label("Current HP:");
            ui.add(egui::DragValue::new(&mut health.current_health));
        });
        ui.horizontal(|ui| {
            ui.label("Max HP:");
            ui.add(egui::DragValue::new(&mut health.max_health));
        });
        ui.checkbox(&mut health.is_dead, "Is Dead");
    });
    if remove {
        entity.health = None;
        entity.animator = None;
        *is_dirty = true;
    }
}

/// 3EE. Collider Component
pub fn draw_collider(
    ui: &mut egui::Ui,
    entity: &mut Entity,
    is_dirty: &mut bool,
    pending_nav_bake: &mut bool,
) {
    if entity.collider.is_none() {
        return;
    }
    let mut remove = false;
    component_card(
        ui,
        icon::BOUNDING_BOX,
        "Collider",
        Some(&mut remove),
        |ui| {
            let Some(collider) = &mut entity.collider else {
                return;
            };
            ui.checkbox(&mut collider.active, "Active");
            ui.checkbox(&mut collider.is_trigger, "Is Trigger");

            let mut shape_type = match &collider.shape {
                ColliderShape::Box { .. } => "Box",
                ColliderShape::Sphere { .. } => "Sphere",
                ColliderShape::Cylinder { .. } => "Cylinder",
            };
            let old_shape_type = shape_type;
            egui::ComboBox::from_label("Shape")
                .selected_text(shape_type)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut shape_type, "Box", "Box");
                    ui.selectable_value(&mut shape_type, "Sphere", "Sphere");
                    ui.selectable_value(&mut shape_type, "Cylinder", "Cylinder");
                });
            if shape_type != old_shape_type {
                collider.shape = match shape_type {
                    "Sphere" => ColliderShape::Sphere { radius: 0.5 },
                    "Cylinder" => ColliderShape::Cylinder {
                        radius: 0.5,
                        height: 1.0,
                    },
                    _ => ColliderShape::Box { size: Vec3::ONE },
                };
                *pending_nav_bake = true;
            }

            match &mut collider.shape {
                ColliderShape::Box { size } => {
                    ui.horizontal(|ui| {
                        ui.label("Size:");
                        drag(ui, &mut size.x);
                        drag(ui, &mut size.y);
                        drag(ui, &mut size.z);
                    });
                }
                ColliderShape::Sphere { radius } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius:");
                        drag(ui, radius);
                    });
                }
                ColliderShape::Cylinder { radius, height } => {
                    ui.horizontal(|ui| {
                        ui.label("Radius:");
                        drag(ui, radius);
                        ui.label("Height:");
                        drag(ui, height);
                    });
                }
            }
        },
    );
    if remove {
        entity.collider = None;
        *is_dirty = true;
    }
}

/// A clamped collider-dimension drag field (shared between the shape variants).
fn drag(ui: &mut egui::Ui, value: &mut f32) {
    ui.add(
        egui::DragValue::new(value)
            .speed(0.05)
            .clamp_range(0.01..=100.0),
    );
}

/// 3EG. RigidBody Component
pub fn draw_rigidbody(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.rigidbody.is_none() {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::CUBE, "RigidBody", Some(&mut remove), |ui| {
        let Some(rb) = &mut entity.rigidbody else {
            return;
        };
        ui.checkbox(&mut rb.active, "Active");
        ui.checkbox(&mut rb.is_kinematic, "Is Kinematic");
        ui.checkbox(&mut rb.use_gravity, "Use Gravity");
        ui.horizontal(|ui| {
            ui.label("Mass:");
            ui.add(
                egui::DragValue::new(&mut rb.mass)
                    .speed(0.05)
                    .clamp_range(0.01..=1000.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Velocity:");
            ui.add(egui::DragValue::new(&mut rb.velocity.x).speed(0.1));
            ui.add(egui::DragValue::new(&mut rb.velocity.y).speed(0.1));
            ui.add(egui::DragValue::new(&mut rb.velocity.z).speed(0.1));
        });
    });
    if remove {
        entity.rigidbody = None;
        *is_dirty = true;
    }
}

/// 3EH. NavMeshAgent Component
pub fn draw_nav_agent(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    if entity.nav_agent.is_none() {
        return;
    }
    let mut remove = false;
    component_card(ui, icon::PATH, "NavMesh Agent", Some(&mut remove), |ui| {
        let Some(agent) = &mut entity.nav_agent else {
            return;
        };
        ui.checkbox(&mut agent.active, "Active");
        clamped(ui, "Speed:", &mut agent.speed, 0.0..=100.0);
        clamped(ui, "Acceleration:", &mut agent.acceleration, 0.0..=100.0);
        clamped(
            ui,
            "Stopping Distance:",
            &mut agent.stopping_distance,
            0.0..=50.0,
        );
        clamped(ui, "Radius:", &mut agent.radius, 0.01..=10.0);
        ui.horizontal(|ui| {
            ui.label("Target:");
            ui.add(egui::DragValue::new(&mut agent.target.x).speed(0.1));
            ui.add(egui::DragValue::new(&mut agent.target.y).speed(0.1));
            ui.add(egui::DragValue::new(&mut agent.target.z).speed(0.1));
        });
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

/// A labelled, clamped drag row (shared by the nav-agent scalar fields).
fn clamped(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.05).clamp_range(range));
    });
}
