use glam::Vec3;
use std::path::Path;

use crate::scene::{ColliderShape, Entity};

/// 3D. Script bindings
pub fn draw_script(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_script = false;
    if let Some(script) = &mut entity.script {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("📜 Lua Script Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove Script").clicked() {
                    remove_script = true;
                }
            });
        });
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut script.path);
        });

        let t = crate::editor::theme::from_ui(ui);
        let exists = Path::new(&script.path).exists();
        if exists {
            ui.colored_label(t.accent_blue, "✔ Loaded and ready to run");
        } else {
            ui.colored_label(t.danger, "❌ File not found!");
        }
    }
    if remove_script {
        entity.script = None;
        *is_dirty = true;
    }
}

/// 3E. Health component
pub fn draw_health(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_health = false;
    if let Some(health) = &mut entity.health {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("❤️ Health Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove Health").clicked() {
                    remove_health = true;
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Current HP:");
            ui.add(egui::DragValue::new(&mut health.current_health));
        });
        ui.horizontal(|ui| {
            ui.label("Max HP:");
            ui.add(egui::DragValue::new(&mut health.max_health));
        });
        ui.checkbox(&mut health.is_dead, "Is Dead");
    }
    if remove_health {
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
    let mut remove_collider = false;
    if let Some(collider) = &mut entity.collider {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("🟢 Collider Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove Collider").clicked() {
                    remove_collider = true;
                }
            });
        });
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
                "Box" => ColliderShape::Box { size: Vec3::ONE },
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
                    ui.add(
                        egui::DragValue::new(&mut size.x)
                            .speed(0.05)
                            .clamp_range(0.01..=100.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut size.y)
                            .speed(0.05)
                            .clamp_range(0.01..=100.0),
                    );
                    ui.add(
                        egui::DragValue::new(&mut size.z)
                            .speed(0.05)
                            .clamp_range(0.01..=100.0),
                    );
                });
            }
            ColliderShape::Sphere { radius } => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    ui.add(
                        egui::DragValue::new(radius)
                            .speed(0.05)
                            .clamp_range(0.01..=100.0),
                    );
                });
            }
            ColliderShape::Cylinder { radius, height } => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    ui.add(
                        egui::DragValue::new(radius)
                            .speed(0.05)
                            .clamp_range(0.01..=100.0),
                    );
                    ui.label("Height:");
                    ui.add(
                        egui::DragValue::new(height)
                            .speed(0.05)
                            .clamp_range(0.01..=100.0),
                    );
                });
            }
        }
    }
    if remove_collider {
        entity.collider = None;
        *is_dirty = true;
    }
}

/// 3EG. RigidBody Component
pub fn draw_rigidbody(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_rigidbody = false;
    if let Some(rb) = &mut entity.rigidbody {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("📦 RigidBody Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑").on_hover_text("Remove RigidBody").clicked() {
                    remove_rigidbody = true;
                }
            });
        });
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
    }
    if remove_rigidbody {
        entity.rigidbody = None;
        *is_dirty = true;
    }
}

/// 3EH. NavMeshAgent Component
pub fn draw_nav_agent(ui: &mut egui::Ui, entity: &mut Entity, is_dirty: &mut bool) {
    let mut remove_nav_agent = false;
    if let Some(agent) = &mut entity.nav_agent {
        ui.separator();
        ui.horizontal(|ui| {
            ui.heading("🛰️ NavMesh Agent Component");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button("🗑")
                    .on_hover_text("Remove NavMesh Agent")
                    .clicked()
                {
                    remove_nav_agent = true;
                }
            });
        });
        ui.checkbox(&mut agent.active, "Active");

        ui.horizontal(|ui| {
            ui.label("Speed:");
            ui.add(
                egui::DragValue::new(&mut agent.speed)
                    .speed(0.05)
                    .clamp_range(0.0..=100.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Acceleration:");
            ui.add(
                egui::DragValue::new(&mut agent.acceleration)
                    .speed(0.05)
                    .clamp_range(0.0..=100.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Stopping Distance:");
            ui.add(
                egui::DragValue::new(&mut agent.stopping_distance)
                    .speed(0.05)
                    .clamp_range(0.0..=50.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Radius:");
            ui.add(
                egui::DragValue::new(&mut agent.radius)
                    .speed(0.05)
                    .clamp_range(0.01..=10.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Target:");
            ui.add(egui::DragValue::new(&mut agent.target.x).speed(0.1));
            ui.add(egui::DragValue::new(&mut agent.target.y).speed(0.1));
            ui.add(egui::DragValue::new(&mut agent.target.z).speed(0.1));
        });

        ui.horizontal(|ui| {
            ui.label(format!(
                "Velocity: [{:.2}, {:.2}, {:.2}]",
                agent.velocity.x, agent.velocity.y, agent.velocity.z
            ));
        });
    }
    if remove_nav_agent {
        entity.nav_agent = None;
        *is_dirty = true;
    }
}
