//! src/editor/inspector/components/lod_group.rs — the LOD Group card (#472).
//!
//! A THIN client (#287): widgets read a snapshot and route every write through the
//! shared `authoring::lod_group` ops the `LODGroup.*` Lua setters call. One row per
//! level (finest first): its screen-height threshold as a percentage, the renderer
//! entities it shows (by name, each removable), an id field to add one, and a button
//! to drop the level.

use egui_phosphor::regular as icon;

use crate::components::{LodGroupComponent, LodLevel};
use crate::ecs::World;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::lod_group as lod_ops;

/// Route one edit through the shared ops and mark the scene dirty.
fn write(world: &mut World, id: u32, is_dirty: &mut bool, op: impl FnOnce(&mut LodGroupComponent)) {
    if let Some(mut g) = world.lod_group_mut(id) {
        op(&mut g);
    }
    *is_dirty = true;
}

/// 3EL. LOD Group Component
pub fn draw_lod_group(ui: &mut egui::Ui, world: &mut World, id: u32, is_dirty: &mut bool) {
    let Some(g) = world.lod_group(id).map(|g| g.clone()) else {
        return;
    };
    let mut remove = false;
    component_card(
        ui,
        icon::STACK_SIMPLE,
        "LOD Group",
        Some(&mut remove),
        |ui| {
            let mut size = g.size;
            let drag = egui::DragValue::new(&mut size)
                .speed(0.05)
                .clamp_range(lod_ops::MIN_SIZE..=f32::MAX);
            if ui
                .horizontal(|ui| {
                    ui.label("Size (m):");
                    ui.add(drag).changed()
                })
                .inner
            {
                write(world, id, is_dirty, |c| lod_ops::set_size(c, size));
            }
            for (i, level) in g.levels.iter().enumerate() {
                ui.separator();
                draw_level(ui, world, id, i, level, is_dirty);
            }
            ui.separator();
            if ui.button(format!("{}  Add Level", icon::PLUS)).clicked() {
                write(world, id, is_dirty, |c| {
                    lod_ops::add_level(c);
                });
            }
        },
    );
    if remove {
        world.set_lod_group(id, None);
        *is_dirty = true;
    }
}

/// One level: threshold, renderers, and its remove button.
fn draw_level(
    ui: &mut egui::Ui,
    world: &mut World,
    id: u32,
    i: usize,
    level: &LodLevel,
    is_dirty: &mut bool,
) {
    let mut percent = level.screen_height * 100.0;
    let (changed, drop_level) = ui
        .horizontal(|ui| {
            ui.strong(format!("LOD{i}"));
            ui.label("down to");
            let drag = egui::DragValue::new(&mut percent)
                .speed(0.1)
                .suffix(" %")
                .clamp_range(0.0..=100.0);
            let changed = ui.add(drag).changed();
            (changed, ui.small_button(icon::TRASH).clicked())
        })
        .inner;
    if changed {
        let h = percent / 100.0;
        write(world, id, is_dirty, |c| lod_ops::set_level_height(c, i, h));
    }
    if drop_level {
        write(world, id, is_dirty, |c| lod_ops::remove_level(c, i));
        return;
    }
    let mut renderers = level.renderers.clone();
    let mut edited = false;
    renderers.retain(|&r| {
        let name = world.name(r).map_or("<missing>".to_string(), |n| n.clone());
        let keep = !ui
            .horizontal(|ui| {
                ui.label(format!("  {name} (#{r})"));
                ui.small_button(icon::X).clicked()
            })
            .inner;
        edited |= !keep;
        keep
    });
    edited |= add_renderer_row(ui, i, &mut renderers);
    if edited {
        write(world, id, is_dirty, |c| {
            lod_ops::set_renderers(c, i, renderers)
        });
    }
}

/// An entity-id field and a button that appends it to `renderers`. The id being
/// typed lives in egui's temp memory, per level. Returns whether one was added.
fn add_renderer_row(ui: &mut egui::Ui, level: usize, renderers: &mut Vec<u32>) -> bool {
    let key = ui.id().with(("lod_add_renderer", level));
    let mut next: u32 = ui.data_mut(|d| d.get_temp(key).unwrap_or(0));
    let added = ui
        .horizontal(|ui| {
            ui.label("  Add renderer id:");
            ui.add(egui::DragValue::new(&mut next));
            ui.small_button(icon::PLUS).clicked()
        })
        .inner;
    ui.data_mut(|d| d.insert_temp(key, next));
    if added && !renderers.contains(&next) {
        renderers.push(next);
        return true;
    }
    false
}
