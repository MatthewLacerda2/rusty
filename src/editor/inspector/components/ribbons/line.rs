//! src/editor/inspector/components/ribbons/line.rs — the Line card (#441).

use egui_phosphor::regular as icon;

use super::draw_style;
use crate::components::LineComponent;
use crate::editor::inspector::components::card::component_card;
use crate::editor::inspector::components::particles::widgets::vec3;
use crate::scene::authoring::line as line_ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(
    world: &mut crate::ecs::World,
    id: u32,
    dirty: &mut bool,
    op: impl FnOnce(&mut LineComponent),
) {
    if let Some(mut l) = world.line_mut(id) {
        op(&mut l);
    }
    *dirty = true;
}

/// 3FL. Line Renderer card: world space, loop, the point list, and the shared
/// Style section.
pub fn draw_line(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, dirty: &mut bool) {
    let Some(l) = world.line(id).map(|l| l.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::LINE_SEGMENTS,
        "Line Renderer",
        Some(&mut remove),
        |ui| {
            let mut world_space = l.use_world_space;
            if ui.checkbox(&mut world_space, "Use World Space").changed() {
                write(world, id, dirty, |c| {
                    line_ops::set_use_world_space(c, world_space)
                });
            }
            let mut looping = l.looping;
            if ui.checkbox(&mut looping, "Loop").changed() {
                write(world, id, dirty, |c| line_ops::set_looping(c, looping));
            }
            if let Some(points) = draw_points(ui, &l.positions) {
                write(world, id, dirty, |c| line_ops::set_positions(c, &points));
            }
            if let Some(edit) = draw_style(ui, &l.style) {
                write(world, id, dirty, |c| edit.apply(&mut c.style));
            }
        },
    );
    if remove {
        world.set_line(id, None);
        *dirty = true;
    }
}

/// One x/y/z row per point with a remove button, and "Add point" (repeating the
/// last). Returns the edited list on change.
fn draw_points(ui: &mut egui::Ui, points: &[glam::Vec3]) -> Option<Vec<glam::Vec3>> {
    let mut edited = points.to_vec();
    let mut changed = false;
    let mut remove = None;
    egui::CollapsingHeader::new(format!("Positions ({})", points.len()))
        .id_source("line_positions")
        .show(ui, |ui| {
            for (i, p) in edited.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    changed |= vec3(ui, &format!("{i}"), p, 0.05);
                    if ui.small_button("x").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if ui.button("Add point").clicked() {
                edited.push(edited.last().copied().unwrap_or_default());
                changed = true;
            }
        });
    if let Some(i) = remove {
        edited.remove(i);
        changed = true;
    }
    changed.then_some(edited)
}
