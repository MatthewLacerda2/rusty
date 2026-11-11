//! src/editor/inspector/components/gameplay/ai_perception.rs — the AI Perception
//! card (#1900).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::ai::perception` ops the `AIPerception.*` Lua setters call.
//! In Play the card also lists what the agent knows, read-only: that memory is the
//! senses' own.

use crate::components::AIPerceptionComponent as Perception;
use crate::editor::inspector::components::card::{component_card, remove_component};
use crate::editor::widgets::DragRange;
use crate::scene::authoring::ai::perception as ops;
use crate::scene::authoring::ComponentKind;
use egui_phosphor::regular as icon;
use std::ops::RangeInclusive;

/// A shared scalar setter, one per drag row.
type Op = fn(&mut Perception, f32);
/// A shared mask setter.
type MaskOp = fn(&mut Perception, u32);

/// Route one edit through the shared ops and mark the scene dirty.
fn write(
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
    op: impl FnOnce(&mut Perception),
) {
    if let Some(mut p) = world.ai_perception_mut(id) {
        op(&mut p);
    }
    *is_dirty = true;
}

/// 3EP. AIPerception Component
pub fn draw_ai_perception(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    is_dirty: &mut bool,
) {
    let Some(p) = world.ai_perception(id).map(|p| p.clone()) else {
        return;
    };
    let d = is_dirty;
    let mut remove = false;
    component_card(ui, icon::EYE, "AI Perception", Some(&mut remove), |ui| {
        draw_distances(ui, world, id, &p, &mut *d);
        draw_ages(ui, world, id, &p, &mut *d);
        draw_masks(ui, world, id, &p, &mut *d);
        draw_known(ui, &p);
    });
    if remove {
        remove_component(world, id, ComponentKind::AIPerception, d);
    }
}

/// One drag row: its label, value, range and shared setter.
type Row<'a> = (&'a str, f32, RangeInclusive<f32>, Op);

/// The ranges, the cone and the eye height, one drag row each.
fn draw_distances(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    p: &Perception,
    d: &mut bool,
) {
    let sight: [Row; 5] = [
        (
            "Sight Range:",
            p.sight_range,
            0.0..=500.0,
            ops::set_sight_range,
        ),
        (
            "Lose Sight Range:",
            p.lose_sight_range,
            0.0..=500.0,
            ops::set_lose_sight_range,
        ),
        (
            "Sight Angle (°):",
            p.sight_angle,
            0.0..=ops::MAX_SIGHT_ANGLE,
            ops::set_sight_angle,
        ),
        (
            "Eye Height:",
            p.eye_height,
            -10.0..=10.0,
            ops::set_eye_height,
        ),
        (
            "Hearing Range:",
            p.hearing_range,
            0.0..=500.0,
            ops::set_hearing_range,
        ),
    ];
    drag_rows(ui, world, id, d, sight);
}

/// The three max ages.
fn draw_ages(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    p: &Perception,
    d: &mut bool,
) {
    let ages: [Row; 3] = [
        (
            "Sight Max Age (s):",
            p.sight_max_age,
            0.0..=600.0,
            ops::set_sight_max_age,
        ),
        (
            "Hearing Max Age (s):",
            p.hearing_max_age,
            0.0..=600.0,
            ops::set_hearing_max_age,
        ),
        (
            "Damage Max Age (s):",
            p.damage_max_age,
            0.0..=600.0,
            ops::set_damage_max_age,
        ),
    ];
    drag_rows(ui, world, id, d, ages);
    ui.label("Max age 0: remembered forever");
}

/// Draw `rows`, routing each edit through its setter.
fn drag_rows<const N: usize>(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    d: &mut bool,
    rows: [Row; N],
) {
    for (label, mut value, range, op) in rows {
        let edited = ui
            .horizontal(|ui| {
                ui.label(label);
                ui.add(egui::DragValue::new(&mut value).speed(0.05).ranged(range))
                    .changed()
            })
            .inner;
        if edited {
            write(world, id, d, |p| op(p, value));
        }
    }
}

/// The sight and blocking layer masks.
fn draw_masks(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    p: &Perception,
    d: &mut bool,
) {
    let masks: [(&str, u32, MaskOp); 2] = [
        ("Sight Mask:", p.sight_mask, ops::set_sight_mask),
        ("Blocking Mask:", p.blocking_mask, ops::set_blocking_mask),
    ];
    for (label, mask, op) in masks {
        if let Some(next) = mask_row(ui, label, mask) {
            write(world, id, d, |p| op(p, next));
        }
    }
}

/// A layer mask as a number with Everything / Nothing shortcuts (bit `i` = layer
/// `i`); the new mask when edited.
fn mask_row(ui: &mut egui::Ui, label: &str, mask: u32) -> Option<u32> {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut value = mask;
        let mut next = None;
        if ui
            .add(egui::DragValue::new(&mut value).hexadecimal(8, false, true))
            .changed()
        {
            next = Some(value);
        }
        if ui.small_button("Everything").clicked() {
            next = Some(u32::MAX);
        }
        if ui.small_button("Nothing").clicked() {
            next = Some(0);
        }
        next
    })
    .inner
}

/// What the agent knows, read-only: one line per known target.
fn draw_known(ui: &mut egui::Ui, p: &Perception) {
    if p.memory.known.is_empty() {
        return;
    }
    ui.separator();
    ui.label("Known (runtime):");
    for (target, k) in &p.memory.known {
        let seen = if k.sensed { " (sensed)" } else { "" };
        ui.label(format!(
            "    #{target} by {}: [{:.1}, {:.1}, {:.1}], {:.1} s{seen}",
            k.sense.name(),
            k.position.x,
            k.position.y,
            k.position.z,
            k.age
        ));
    }
}

impl crate::editor::inspector::components::add::AddEntry
    for crate::components::AIPerceptionComponent
{
    const CATEGORY: crate::editor::inspector::components::add::Category =
        crate::editor::inspector::components::add::Category::Ai;
    const LABEL: &'static str = "AI Perception";
    const ICON: &'static str = egui_phosphor::regular::EYE;
}
