//! The Material card's table-driven rows: the map coordinates (#1573) and the
//! transparent surface (#1523), one widget per row of a shared
//! `authoring::material` field table that its `shown` admits (a combo box, a
//! checkbox, a slider, a colour button or a pair of drag values, by the row's kind).
//! Every change goes through the shared `set_surface` op `Material.Set<Field>` calls
//! too.

use std::collections::BTreeMap;

use crate::widgets::DragRange;

use rusty_scene::authoring::material::{self as mat_ops, SurfaceField, SurfaceKind, SurfaceValue};
use rusty_scene::MaterialAsset;

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
        if let Some(value) = draw_field(ui, field, &materials[key]) {
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
    material: &MaterialAsset,
) -> Option<SurfaceValue> {
    let value = (field.get)(material);
    ui.horizontal(|ui| {
        let label = ui.label(format!("{}:", field.label));
        if let Some(hint) = (field.hint)(material) {
            label.on_hover_text(hint);
        }
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
                .add(crate::widgets::slider(&mut n, range.clone()))
                .changed()
                .then_some(SurfaceValue::Number(n)),
            (SurfaceKind::Color(range), SurfaceValue::Color(mut rgb)) if *range.end() <= 1.0 => ui
                .color_edit_button_rgb(&mut rgb)
                .changed()
                .then_some(SurfaceValue::Color(rgb)),
            // A tint that can pass 1 (#1998) is three numbers: a picker stops at white.
            (SurfaceKind::Color(range), SurfaceValue::Color(mut rgb)) => {
                drags(ui, &mut rgb, range).then_some(SurfaceValue::Color(rgb))
            }
            (SurfaceKind::Vec2(range), SurfaceValue::Vec2(mut v)) => {
                drags(ui, &mut v, range).then_some(SurfaceValue::Vec2(v))
            }
            _ => None,
        }
    })
    .inner
}

/// One drag per value, each held in `range`; whether any changed.
fn drags(ui: &mut egui::Ui, values: &mut [f32], range: &std::ops::RangeInclusive<f32>) -> bool {
    let mut changed = false;
    for v in values {
        let drag = egui::DragValue::new(v).speed(0.01).ranged(range.clone());
        changed |= ui.add(drag).changed();
    }
    changed
}
