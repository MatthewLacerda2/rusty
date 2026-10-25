//! The Layout Element inspector card (#421): ignore-layout, the min / preferred /
//! flexible size overrides (each axis toggled on to override the content's
//! size) and the content fitter per axis. Widgets edit a snapshot; every write
//! routes through `scene::authoring::ui::layout::element`.

use crate::editor::inspector::components::card::remove_component;
use crate::editor::widgets::DragRange;
use crate::scene::authoring::ComponentKind;
use egui_phosphor::regular as icon;

use crate::components::{LayoutAxisFit, LayoutElementComponent};
use crate::editor::inspector::components::card::component_card;
use crate::editor::inspector::components::ui::combo;
use crate::scene::authoring::ui::layout::element::{self as ops, SizeKind};

/// Layout Element card. A THIN client (#287): widgets edit a snapshot and the
/// changed snapshot is written back through the shared ops; remove detaches.
pub fn draw(ui: &mut egui::Ui, world: &mut crate::ecs::World, id: u32, is_dirty: &mut bool) {
    let Some(mut edit) = world.layout_element(id).map(|e| e.clone()) else {
        return;
    };
    let mut sizes = SizeKind::ALL.map(|k| ops::size(&edit, k));
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
            for (kind, (w, h)) in SizeKind::ALL.iter().zip(sizes.iter_mut()) {
                ui.horizontal(|ui| {
                    ui.label(format!("{kind}:"));
                    changed |= optional(ui, "w", w);
                    changed |= optional(ui, "h", h);
                });
            }
            let fits = LayoutAxisFit::ALL;
            let name = LayoutAxisFit::name;
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
        remove_component(world, id, ComponentKind::LayoutElement, is_dirty);
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
            .add(egui::DragValue::new(&mut v).ranged(0.0..=f32::MAX))
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
    for (kind, (w, h)) in SizeKind::ALL.iter().zip(sizes) {
        ops::set_size(e, *kind, w, h);
    }
    ops::set_fit(e, edit.horizontal_fit, edit.vertical_fit);
}
