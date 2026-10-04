//! src/editor/inspector/components/particles/widgets.rs — the card's labelled
//! rows: clamped numbers, vectors, combos, `[min, max]` ranges, keyframe lists and
//! colour gradients. The Trail and Line cards (#441) reuse them.

use crate::core::curve::{ColorKey, Curve, Gradient, Key, Range};

/// A labelled, clamped `f32` drag row. Returns whether the value changed.
pub(in crate::editor::inspector::components) fn clamped(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(0.05).range(range))
            .changed()
    })
    .inner
}

/// A labelled, clamped `u32` drag row. Returns whether the value changed.
pub(super) fn drag_u32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(1.0).range(range))
            .changed()
    })
    .inner
}

/// A labelled x/y/z drag row over a `Vec3`. Returns whether any axis changed.
pub(in crate::editor::inspector::components) fn vec3(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut glam::Vec3,
    speed: f32,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut c = ui
            .add(egui::DragValue::new(&mut value.x).speed(speed))
            .changed();
        c |= ui
            .add(egui::DragValue::new(&mut value.y).speed(speed))
            .changed();
        c |= ui
            .add(egui::DragValue::new(&mut value.z).speed(speed))
            .changed();
        c
    })
    .inner
}

/// A combo box over a small set of `Copy + PartialEq` enum variants; returns whether it changed.
pub(in crate::editor::inspector::components) fn combo<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> bool {
    let selected = options
        .iter()
        .find(|(v, _)| *v == *value)
        .map(|(_, name)| *name)
        .unwrap_or("");
    let mut changed = false;
    egui::ComboBox::from_label(label)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (variant, name) in options {
                if ui.selectable_label(*value == *variant, *name).clicked() {
                    *value = *variant;
                    changed = true;
                }
            }
        });
    changed
}

/// A labelled `[min, max]` row (a constant range shows both equal). Returns the
/// edited range when either bound changed.
pub(super) fn range_row(
    ui: &mut egui::Ui,
    label: &str,
    range: Range,
    bounds: std::ops::RangeInclusive<f32>,
) -> Option<Range> {
    let (mut min, mut max) = (range.min, range.max);
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            let drag = |v| egui::DragValue::new(v).speed(0.05).range(bounds.clone());
            let a = ui.add(drag(&mut min)).on_hover_text("min").changed();
            let b = ui.add(drag(&mut max)).on_hover_text("max").changed();
            a | b
        })
        .inner;
    changed.then(|| Range::new(min, max))
}

/// A collapsible keyframe list: one `t` / value row per key, a remove button each
/// and an "Add key" button. Returns the edited curve (keys re-sorted) on change.
pub(in crate::editor::inspector::components) fn curve_editor(
    ui: &mut egui::Ui,
    label: &str,
    curve: &Curve,
) -> Option<Curve> {
    let mut keys = curve.keys.clone();
    let mut changed = false;
    egui::CollapsingHeader::new(label)
        .id_salt(label)
        .show(ui, |ui| {
            changed = key_rows(ui, &mut keys);
            if ui.button("Add key").clicked() {
                let value = keys.last().map_or(1.0, |k| k.value);
                keys.push(Key { t: 1.0, value });
                changed = true;
            }
        });
    changed.then(|| Curve::from_keys(&keys.iter().map(|k| (k.t, k.value)).collect::<Vec<_>>()))
}

/// The `t` / value / remove rows of a key list. Returns whether any changed.
fn key_rows(ui: &mut egui::Ui, keys: &mut Vec<Key>) -> bool {
    let mut changed = false;
    let mut remove = None;
    for (i, key) in keys.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label("t");
            let t = egui::DragValue::new(&mut key.t)
                .speed(0.01)
                .range(0.0..=1.0);
            changed |= ui.add(t).changed();
            ui.label("value");
            changed |= ui
                .add(egui::DragValue::new(&mut key.value).speed(0.05))
                .changed();
            if ui.small_button("x").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        keys.remove(i);
        changed = true;
    }
    changed
}

/// Colour keys (t + RGB, removable, addable) and the alpha curve.
pub(in crate::editor::inspector::components) fn gradient_editor(
    ui: &mut egui::Ui,
    label: &str,
    gradient: &Gradient,
) -> Option<Gradient> {
    let mut g = gradient.clone();
    let mut changed = false;
    egui::CollapsingHeader::new(label).show(ui, |ui| {
        let mut remove = None;
        for (i, key) in g.color_keys.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label("t");
                let t = egui::DragValue::new(&mut key.t)
                    .speed(0.01)
                    .range(0.0..=1.0);
                changed |= ui.add(t).changed();
                changed |= ui.color_edit_button_rgb(&mut key.color).changed();
                if ui.small_button("x").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            g.color_keys.remove(i);
            changed = true;
        }
        if ui.button("Add color key").clicked() {
            g.color_keys.push(ColorKey {
                t: 1.0,
                color: [1.0; 3],
            });
            changed = true;
        }
        if let Some(alpha) = curve_editor(ui, "Alpha", &g.alpha) {
            g.alpha = alpha;
            changed = true;
        }
    });
    changed.then_some(g)
}
