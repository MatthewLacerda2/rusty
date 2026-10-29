//! The Material card's table-driven rows: the map coordinates (#1573) and the
//! transparent surface (#1523), one widget per row of a shared
//! `authoring::material` field table that its `shown` admits (a combo box, a
//! checkbox, a slider, a colour button or a pair of drag values, by the row's kind).
//! Every change goes through the shared `set_surface` op `Material.Set<Field>` calls
//! too.

use std::collections::BTreeMap;

use crate::scene::authoring::material::{self as mat_ops, SurfaceField, SurfaceKind, SurfaceValue};
use crate::scene::MaterialAsset;

/// Draw every row of `fields` material `key` shows.
pub(super) fn draw_fields(
    ui: &mut egui::Ui,
    fields: &'static [SurfaceField],
    materials: &mut BTreeMap<String, MaterialAsset>,
    key: &str,
    is_dirty: &mut bool,
) {
    for field in fields {
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
            (SurfaceKind::Vec2(range), SurfaceValue::Vec2(mut v)) => {
                let x = ui.add(
                    egui::DragValue::new(&mut v[0])
                        .speed(0.01)
                        .range(range.clone()),
                );
                let y = ui.add(
                    egui::DragValue::new(&mut v[1])
                        .speed(0.01)
                        .range(range.clone()),
                );
                (x.changed() || y.changed()).then_some(SurfaceValue::Vec2(v))
            }
            _ => None,
        }
    })
    .inner
}
