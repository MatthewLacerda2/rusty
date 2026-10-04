//! The Layout Element inspector card (#421): ignore-layout, the min / preferred /
//! flexible size overrides (each axis toggled on to override the content's
//! size) and the content fitter per axis. Widgets edit a snapshot; every write
//! routes through `scene::authoring::layout_element`.

use egui_phosphor::regular as icon;

use super::combo;
use crate::components::LayoutElementComponent;
use crate::editor::inspector::components::card::component_card;
use crate::scene::authoring::layout_element::{self as ops, SizeKind, FITS};
use crate::scene::authoring::layout_group::name_of;

const SIZES: [(SizeKind, &str); 3] = [
    (SizeKind::Min, "Min"),
    (SizeKind::Preferred, "Preferred"),
    (SizeKind::Flexible, "Flexible"),
];

/// Layout Element card. A THIN client (#287): widgets edit a snapshot and the
/// changed snapshot is written back through the shared ops; remove detaches.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut edit) = world.layout_element(id).map(|e| e.clone()) else {
        return;
    };
    let mut sizes = SIZES.map(|(k, _)| ops::size(&edit, k));
    let mut remove = false;
    let mut changed = false;
    component_card(
        ui,
        icon::ARROWS_OUT,
        "Layout Element",
        Some(&mut remove),
        |ui| {
            changed |= ui
                .checkbox(&mut edit.ignore_layout, "Ignore Layout")
                .changed();
            for ((_, label), (w, h)) in SIZES.iter().zip(sizes.iter_mut()) {
                ui.horizontal(|ui| {
                    ui.label(format!("{label}:"));
                    changed |= optional(ui, "w", w);
                    changed |= optional(ui, "h", h);
                });
            }
            let fits = FITS.map(|(f, _)| f);
            let name = |f| name_of(&FITS, f);
            changed |= combo(ui, "Horizontal Fit", &mut edit.horizontal_fit, &fits, name);
            changed |= combo(ui, "Vertical Fit", &mut edit.vertical_fit, &fits, name);
        },
    );
    if changed {
        if let Some(mut e) = world.layout_element_mut(id) {
            write_back(&mut e, &edit, sizes);
        }
    }
    if remove {
        world.set_layout_element(id, None);
    }
    *is_dirty |= remove || changed;
}

/// A checkbox that turns an override on, then its value.
fn optional(ui: &mut egui::Ui, label: &str, value: &mut Option<f32>) -> bool {
    let mut on = value.is_some();
    let mut v = value.unwrap_or(0.0);
    let mut changed = ui.checkbox(&mut on, label).changed();
    if on {
        changed |= ui
            .add(egui::DragValue::new(&mut v).range(0.0..=f32::MAX))
            .changed();
    }
    *value = on.then_some(v);
    changed
}

/// Write the edited snapshot back through the shared ops.
fn write_back(
    e: &mut LayoutElementComponent,
    edit: &LayoutElementComponent,
    sizes: [(Option<f32>, Option<f32>); 3],
) {
    ops::set_ignore_layout(e, edit.ignore_layout);
    for ((kind, _), (w, h)) in SIZES.iter().zip(sizes) {
        ops::set_size(e, *kind, w, h);
    }
    ops::set_fit(e, edit.horizontal_fit, edit.vertical_fit);
}
