//! The Canvas inspector card (#417): render mode, sort order and the *Scale With
//! Screen Size* scaler. Every write routes through `scene::authoring::canvas`.

use egui_phosphor::regular as icon;

use super::vec2_row;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::canvas as canvas_ops;

/// Canvas card. A THIN client (#287): widgets read a snapshot and route each write
/// through a shared op; remove detaches the component.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(c) = world.canvas(id).map(|c| c.clone()) else {
        return;
    };
    let mut remove = false;
    let mut changed = false;
    component_card(ui, icon::MONITOR, "Canvas", Some(&mut remove), |ui| {
        ui.label(format!(
            "Render Mode: {}",
            canvas_ops::render_mode_name(c.render_mode)
        ));
        let mut order = c.sort_order;
        let edited = ui
            .horizontal(|ui| {
                ui.label("Sort Order:");
                ui.add(egui::DragValue::new(&mut order)).changed()
            })
            .inner;
        if edited {
            if let Some(mut c) = world.canvas_mut(id) {
                canvas_ops::set_sort_order(&mut c, order);
            }
            changed = true;
        }
        ui.separator();
        changed |= draw_scaler(ui, world, id, &c);
    });
    if remove {
        world.set_canvas(id, None);
    }
    *is_dirty |= remove || changed;
}

/// Reference resolution + the width/height match slider.
fn draw_scaler(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    c: &crate::components::CanvasComponent,
) -> bool {
    ui.label("Scale With Screen Size");
    let mut changed = false;
    if let Some(res) = vec2_row(ui, "Reference:", c.reference_resolution, 1.0) {
        if let Some(mut c) = world.canvas_mut(id) {
            canvas_ops::set_reference_resolution(&mut c, res);
        }
        changed = true;
    }
    let mut m = c.match_width_or_height;
    let edited = ui
        .horizontal(|ui| {
            ui.label("Match (width ↔ height):");
            ui.add(egui::Slider::new(&mut m, 0.0..=1.0)).changed()
        })
        .inner;
    if edited {
        if let Some(mut c) = world.canvas_mut(id) {
            canvas_ops::set_match_width_or_height(&mut c, m);
        }
        changed = true;
    }
    changed
}
