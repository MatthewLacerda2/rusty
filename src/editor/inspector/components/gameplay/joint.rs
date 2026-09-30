//! src/editor/inspector/components/gameplay/joint.rs — the Joint card (#449).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::joint` ops the `Joint.*` Lua setters call. Rows that do not
//! apply to the chosen kind (the axis and limits of a Fixed joint, the swing of a
//! Hinge) are hidden, not cleared, so switching kinds back loses nothing.

use egui_phosphor::regular as icon;

use super::physics::vec3_row;
use crate::components::{JointComponent, JointKind};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::joint as joint_ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    op: impl FnOnce(&mut JointComponent),
) {
    if let Some(mut j) = world.joint_mut(id) {
        op(&mut j);
    }
    *is_dirty = true;
}

/// 3EJ. Joint Component
pub fn draw_joint(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(j) = world.joint(id).map(|j| j.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(ui, icon::LINK, "Joint", Some(&mut remove), |ui| {
        draw_kind(ui, world, id, j.kind, is_dirty);
        draw_bodies(ui, world, id, &j, is_dirty);
        if j.kind != JointKind::Fixed {
            draw_limits(ui, world, id, &j, is_dirty);
        }
        draw_breaking(ui, world, id, &j, is_dirty);
    });
    if remove {
        world.set_joint(id, None);
        *is_dirty = true;
    }
}

/// The Fixed / Hinge / Ball combo.
fn draw_kind(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    current: JointKind,
    is_dirty: &mut bool,
) {
    let mut kind = current;
    egui::ComboBox::from_label("Kind")
        .selected_text(kind.name())
        .show_ui(ui, |ui| {
            for k in JointKind::ALL {
                ui.selectable_value(&mut kind, k, k.name());
            }
        });
    if kind != current {
        write(world, id, is_dirty, |c| joint_ops::set_kind(c, kind));
    }
}

/// Connected body, anchors and the collision flag.
fn draw_bodies(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    j: &JointComponent,
    is_dirty: &mut bool,
) {
    let mut connected = j.connected_body.is_some();
    let mut body = j.connected_body.unwrap_or(0);
    let changed = ui
        .horizontal(|ui| {
            let c = ui.checkbox(&mut connected, "Connected Body").changed();
            c | ui
                .add_enabled(connected, egui::DragValue::new(&mut body))
                .changed()
        })
        .inner;
    if changed {
        let target = connected.then_some(body);
        write(world, id, is_dirty, |c| {
            joint_ops::set_connected_body(c, target)
        });
    }
    if let Some(v) = vec3_row(ui, "Anchor:", j.anchor) {
        write(world, id, is_dirty, |c| joint_ops::set_anchor(c, v));
    }
    let mut auto = j.auto_configure_connected_anchor;
    if ui
        .checkbox(&mut auto, "Auto Configure Connected Anchor")
        .changed()
    {
        write(world, id, is_dirty, |c| {
            joint_ops::set_auto_configure_connected_anchor(c, auto)
        });
    }
    if !auto {
        if let Some(v) = vec3_row(ui, "Connected Anchor:", j.connected_anchor) {
            write(world, id, is_dirty, |c| {
                joint_ops::set_connected_anchor(c, v)
            });
        }
    }
    let mut collide = j.enable_collision;
    if ui.checkbox(&mut collide, "Enable Collision").changed() {
        write(world, id, is_dirty, |c| {
            joint_ops::set_enable_collision(c, collide)
        });
    }
}

/// Axis and angle limits (Hinge and Ball).
fn draw_limits(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    j: &JointComponent,
    is_dirty: &mut bool,
) {
    if let Some(v) = vec3_row(ui, "Axis:", j.axis) {
        write(world, id, is_dirty, |c| joint_ops::set_axis(c, v));
    }
    let mut use_limits = j.use_limits;
    if ui.checkbox(&mut use_limits, "Use Limits").changed() {
        write(world, id, is_dirty, |c| {
            joint_ops::set_use_limits(c, use_limits)
        });
    }
    if !use_limits {
        return;
    }
    let (mut min, mut max) = (j.limits.x, j.limits.y);
    let range = -joint_ops::MAX_ANGLE..=joint_ops::MAX_ANGLE;
    let changed = ui
        .horizontal(|ui| {
            ui.label("Limits (deg):");
            let drag = |v| egui::DragValue::new(v).clamp_range(range.clone());
            ui.add(drag(&mut min)).changed() | ui.add(drag(&mut max)).changed()
        })
        .inner;
    if changed {
        write(world, id, is_dirty, |c| joint_ops::set_limits(c, min, max));
    }
    if j.kind == JointKind::Ball {
        let mut swing = j.swing_limit;
        let drag = egui::DragValue::new(&mut swing).clamp_range(0.0..=joint_ops::MAX_ANGLE);
        if ui
            .horizontal(|ui| {
                ui.label("Swing Limit (deg):");
                ui.add(drag).changed()
            })
            .inner
        {
            write(world, id, is_dirty, |c| {
                joint_ops::set_swing_limit(c, swing)
            });
        }
    }
}

/// Break force and torque (0 = unbreakable).
fn draw_breaking(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    j: &JointComponent,
    is_dirty: &mut bool,
) {
    let mut force = j.break_force;
    let mut torque = j.break_torque;
    let row = |ui: &mut egui::Ui, label: &str, v: &mut f32| {
        ui.horizontal(|ui| {
            ui.label(label);
            let drag = egui::DragValue::new(v)
                .speed(1.0)
                .clamp_range(0.0..=f32::MAX);
            ui.add(drag).changed()
        })
        .inner
    };
    if row(ui, "Break Force (0 = never):", &mut force) {
        write(world, id, is_dirty, |c| {
            joint_ops::set_break_force(c, force)
        });
    }
    if row(ui, "Break Torque (0 = never):", &mut torque) {
        write(world, id, is_dirty, |c| {
            joint_ops::set_break_torque(c, torque)
        });
    }
}
