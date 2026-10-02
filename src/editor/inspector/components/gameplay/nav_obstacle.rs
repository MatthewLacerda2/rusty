//! src/editor/inspector/components/gameplay/nav_obstacle.rs — the NavMesh
//! Obstacle card (#456).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::nav_obstacle` ops the `NavMeshObstacle.*` Lua setters call.
//! A carving obstacle changes the navmesh, so every edit asks for a nav rebake
//! (incremental: only the obstacle's cells).

use egui_phosphor::regular as icon;

use super::physics::vec3_row;
use crate::components::{NavMeshObstacleComponent as Obstacle, ObstacleShape};
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::nav_obstacle as obstacle_ops;

/// Route one edit through the shared ops, mark the scene dirty, ask for a rebake.
fn write(
    world: &mut crate::ecs::World,
    id: u32,
    flags: (&mut bool, &mut bool),
    op: impl FnOnce(&mut Obstacle),
) {
    if let Some(mut o) = world.nav_obstacle_mut(id) {
        op(&mut o);
    }
    *flags.0 = true;
    *flags.1 = true;
}

/// One labelled drag row; `Some(new)` when it was edited.
fn drag_row(ui: &mut egui::Ui, label: &str, mut value: f32, speed: f64) -> Option<f32> {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(&mut value).speed(speed))
            .changed()
            .then_some(value)
    })
    .inner
}

/// 3EL. NavMeshObstacle Component
pub fn draw_nav_obstacle(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    pending_nav_bake: &mut bool,
) {
    let Some(o) = world.nav_obstacle(id).map(|o| o.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::TRAFFIC_CONE,
        "NavMesh Obstacle",
        Some(&mut remove),
        |ui| {
            let mut active = o.active;
            if ui.checkbox(&mut active, "Active").changed() {
                write(world, id, (is_dirty, pending_nav_bake), |o| {
                    obstacle_ops::set_active(o, active)
                });
            }
            draw_shape(ui, world, id, &o, (is_dirty, pending_nav_bake));
            draw_carving(ui, world, id, &o, (is_dirty, pending_nav_bake));
            ui.label(format!("Carving now: {}", o.is_carving()));
        },
    );
    if remove {
        world.set_nav_obstacle(id, None);
        *is_dirty = true;
        *pending_nav_bake = true;
    }
}

/// Shape selector, centre, and the box size or capsule radius/height.
fn draw_shape(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    o: &Obstacle,
    f: (&mut bool, &mut bool),
) {
    let (d, b) = f;
    let mut shape = o.shape;
    egui::ComboBox::from_label("Shape")
        .selected_text(shape.as_str())
        .show_ui(ui, |ui| {
            for k in ObstacleShape::ALL {
                ui.selectable_value(&mut shape, k, k.as_str());
            }
        });
    if shape != o.shape {
        write(world, id, (&mut *d, &mut *b), |o| {
            obstacle_ops::set_shape(o, shape)
        });
    }
    if let Some(v) = vec3_row(ui, "Center:", o.center) {
        write(world, id, (&mut *d, &mut *b), |o| {
            obstacle_ops::set_center(o, v)
        });
    }
    match o.shape {
        ObstacleShape::Box => {
            if let Some(v) = vec3_row(ui, "Size:", o.size) {
                write(world, id, (d, b), |o| obstacle_ops::set_size(o, v));
            }
        }
        ObstacleShape::Capsule => {
            if let Some(v) = drag_row(ui, "Radius:", o.radius, 0.02) {
                write(world, id, (&mut *d, &mut *b), |o| {
                    obstacle_ops::set_radius(o, v)
                });
            }
            if let Some(v) = drag_row(ui, "Height:", o.height, 0.05) {
                write(world, id, (d, b), |o| obstacle_ops::set_height(o, v));
            }
        }
    }
}

/// Carve, carve-only-stationary and its two thresholds.
fn draw_carving(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    o: &Obstacle,
    f: (&mut bool, &mut bool),
) {
    let (d, b) = f;
    let mut carve = o.carve;
    if ui.checkbox(&mut carve, "Carve").changed() {
        write(world, id, (&mut *d, &mut *b), |o| {
            obstacle_ops::set_carve(o, carve)
        });
    }
    if !o.carve {
        return;
    }
    let mut only = o.carve_only_stationary;
    if ui.checkbox(&mut only, "Carve Only Stationary").changed() {
        write(world, id, (&mut *d, &mut *b), |o| {
            obstacle_ops::set_carve_only_stationary(o, only)
        });
    }
    if let Some(v) = drag_row(ui, "Move Threshold:", o.move_threshold, 0.01) {
        write(world, id, (&mut *d, &mut *b), |o| {
            obstacle_ops::set_move_threshold(o, v)
        });
    }
    if let Some(v) = drag_row(ui, "Time To Stationary:", o.time_to_stationary, 0.05) {
        write(world, id, (d, b), |o| {
            obstacle_ops::set_time_to_stationary(o, v)
        });
    }
}
