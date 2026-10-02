//! src/editor/inspector/components/gameplay/character_controller.rs — the
//! CharacterController card (#451).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::character_controller` ops the `CharacterController.*` Lua
//! setters call. The grounded state is what the last `Move` found, so it is shown
//! read-only.

use egui_phosphor::regular as icon;

use super::physics::vec3_row;
use crate::components::CharacterControllerComponent as Cc;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::character_controller as cc_ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool, op: impl FnOnce(&mut Cc)) {
    if let Some(mut c) = world.character_controller_mut(id) {
        op(&mut c);
    }
    *is_dirty = true;
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

/// 3EK. CharacterController Component
pub fn draw_character_controller(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(c) = world.character_controller(id).map(|c| c.clone()) else {
        return;
    };
    let mut remove = false;
    let title = "Character Controller";
    component_card(
        ui,
        icon::PERSON_SIMPLE_WALK,
        title,
        Some(&mut remove),
        |ui| {
            draw_capsule(ui, world, id, &c, is_dirty);
            draw_tuning(ui, world, id, &c, is_dirty);
            ui.label(format!("Grounded: {}", c.is_grounded));
        },
    );
    if remove {
        world.set_character_controller(id, None);
        *is_dirty = true;
    }
}

/// Height, radius and centre.
fn draw_capsule(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, c: &Cc, d: &mut bool) {
    if let Some(v) = drag_row(ui, "Height:", c.height, 0.05) {
        write(world, id, d, |c| cc_ops::set_height(c, v));
    }
    if let Some(v) = drag_row(ui, "Radius:", c.radius, 0.02) {
        write(world, id, d, |c| cc_ops::set_radius(c, v));
    }
    if let Some(v) = vec3_row(ui, "Center:", c.center) {
        write(world, id, d, |c| cc_ops::set_center(c, v));
    }
}

/// Step offset, slope limit, skin width and minimum move distance.
fn draw_tuning(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, c: &Cc, d: &mut bool) {
    if let Some(v) = drag_row(ui, "Step Offset:", c.step_offset, 0.02) {
        write(world, id, d, |c| cc_ops::set_step_offset(c, v));
    }
    if let Some(v) = drag_row(ui, "Slope Limit:", c.slope_limit, 0.5) {
        write(world, id, d, |c| cc_ops::set_slope_limit(c, v));
    }
    if let Some(v) = drag_row(ui, "Skin Width:", c.skin_width, 0.005) {
        write(world, id, d, |c| cc_ops::set_skin_width(c, v));
    }
    if let Some(v) = drag_row(ui, "Min Move Distance:", c.min_move_distance, 0.001) {
        write(world, id, d, |c| cc_ops::set_min_move_distance(c, v));
    }
}
