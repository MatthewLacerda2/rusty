//! The Material card's transparent-surface rows (#1523): one widget per row of the
//! shared `authoring::material::SURFACE_FIELDS` table that its `shown` admits (a
//! combo box, a checkbox, a slider or a colour button, by the row's kind). Every
//! change goes through the shared `set_surface` op `Material.Set<Field>` calls too.

use std::collections::BTreeMap;

use crate::scene::authoring::material::{
    self as mat_ops, SurfaceField, SurfaceKind, SurfaceValue, SURFACE_FIELDS,
};
use crate::scene::MaterialAsset;

/// Draw every surface row material `key` shows.
pub(super) fn draw_surface_fields(
    ui: &mut egui::Ui,
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    is_dirty: &mut bool,
) {
    for field in SURFACE_FIELDS {
        if !(field.shown)(&materials[key]) {
            continue;
        }
        if let Some(value) = draw_field(ui, field, (field.get)(&materials[key])) {
            // A widget only offers values of the row's own kind, so it is never refused.
            if mat_ops::set_surface(materials, key, field, value).is_ok() {
                *is_dirty = true;
            }
        }
    }
}

/// One labelled widget over `value`; the new value when the user changed it.
fn draw_field(
    ui: &mut egui::Ui,
    field: &SurfaceField,
    value: SurfaceValue,
) -> Option<SurfaceValue> {
    ui.horizontal(|ui| {
        ui.label(format!("{}:", field.label));
        match (&field.kind, value) {
            (SurfaceKind::Choice(names), SurfaceValue::Text(mut current)) => {
                let mut changed = false;
                egui::ComboBox::from_id_salt(field.key)
                    .selected_text(current.clone())
                    .show_ui(ui, |ui| {
                        for name in names() {
                            let picked = ui.selectable_value(&mut current, name.to_owned(), name);
                            changed |= picked.changed();
                        }
                    });
                changed.then_some(SurfaceValue::Text(current))
            }
            (SurfaceKind::Toggle, SurfaceValue::Bool(mut on)) => ui
                .checkbox(&mut on, "")
                .changed()
                .then_some(SurfaceValue::Bool(on)),
            (SurfaceKind::Number(range), SurfaceValue::Number(mut n)) => ui
                .add(crate::editor::widgets::slider(&mut n, range.clone()))
                .changed()
                .then_some(SurfaceValue::Number(n)),
            (SurfaceKind::Color, SurfaceValue::Color(mut rgb)) => ui
                .color_edit_button_rgb(&mut rgb)
                .changed()
                .then_some(SurfaceValue::Color(rgb)),
            _ => None,
        }
    })
    .inner
}
