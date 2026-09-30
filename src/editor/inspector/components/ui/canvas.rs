//! The Canvas inspector card (#417): render mode, sort order and the *Scale With
//! Screen Size* scaler, plus the world / camera canvas knobs of the chosen mode
//! (#429). Every write routes through `scene::authoring::canvas`.

use egui_phosphor::regular as icon;

use super::{combo, vec2_row};
use crate::components::{CanvasComponent, CanvasRenderMode};
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
        let mut mode = c.render_mode;
        let modes = &canvas_ops::RENDER_MODES;
        if combo(
            ui,
            "Render Mode",
            &mut mode,
            modes,
            canvas_ops::render_mode_name,
        ) {
            if let Some(mut c) = world.canvas_mut(id) {
                canvas_ops::set_render_mode(&mut c, mode);
            }
            changed = true;
        }
        changed |= draw_placement(ui, world, id, &c);
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

/// A labelled scalar drag row. Returns the edited value when it changed.
fn scalar_row(ui: &mut egui::Ui, label: &str, value: f32, speed: f32) -> Option<f32> {
    let mut v = value;
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            ui.add(egui::DragValue::new(&mut v).speed(speed)).changed()
        })
        .inner;
    changed.then_some(v)
}

/// The chosen mode's placement: `WorldSpace` density, or the `ScreenSpaceCamera`
/// plane's distance, tilt and sway.
fn draw_placement(
    ui: &mut egui::Ui,
    world: &mut crate::ecs::World,
    id: u32,
    c: &CanvasComponent,
) -> bool {
    type Write = Box<dyn Fn(&mut CanvasComponent)>;
    let mut write: Option<Write> = None;
    match c.render_mode {
        CanvasRenderMode::ScreenSpaceOverlay => {}
        CanvasRenderMode::WorldSpace => {
            if let Some(v) = scalar_row(ui, "Pixels / Unit:", c.pixels_per_unit, 1.0) {
                write = Some(Box::new(move |c| canvas_ops::set_pixels_per_unit(c, v)));
            }
        }
        CanvasRenderMode::ScreenSpaceCamera => {
            if let Some(v) = scalar_row(ui, "Plane Distance:", c.plane_distance, 0.01) {
                write = Some(Box::new(move |c| canvas_ops::set_plane_distance(c, v)));
            }
            if let Some(v) = vec2_row(ui, "Tilt:", c.tilt, 0.5) {
                write = Some(Box::new(move |c| canvas_ops::set_tilt(c, v)));
            }
            let mut sway = c.sway;
            let edited = ui
                .horizontal(|ui| {
                    ui.label("Sway:");
                    ui.add(egui::Slider::new(&mut sway, 0.0..=1.0)).changed()
                })
                .inner;
            if edited {
                write = Some(Box::new(move |c| canvas_ops::set_sway(c, sway)));
            }
        }
    }
    let Some(write) = write else {
        return false;
    };
    if let Some(mut c) = world.canvas_mut(id) {
        write(&mut c);
    }
    true
}
