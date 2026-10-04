//! Widgets the UI graphic cards share (#425): a colour row, a drag row, the blend
//! mode picker and the gradient editor (kind, direction or centre/radius, 2–4
//! stops). They edit a snapshot; the card writes it back through its ops.

use glam::{Vec2, Vec4};

use super::{combo, vec2_row};
use crate::components::{GradientKind, GradientStop, UiBlend, UiGradient, MAX_GRADIENT_STOPS};
use crate::scene::authoring::ui_look::{blend_name, gradient_kind_name};

/// A labelled colour button (straight alpha).
pub(super) fn color_row(ui: &mut egui::Ui, label: &str, color: &mut Vec4) -> bool {
    let mut c = color.to_array();
    let changed = ui
        .horizontal(|ui| {
            ui.label(label);
            ui.color_edit_button_rgba_unmultiplied(&mut c).changed()
        })
        .inner;
    *color = Vec4::from_array(c);
    changed
}

/// A labelled drag value.
pub(super) fn drag_row(ui: &mut egui::Ui, label: &str, value: &mut f32, speed: f32) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(value).speed(speed)).changed()
    })
    .inner
}

/// The blend-mode combo.
pub(super) fn blend_row(ui: &mut egui::Ui, blend: &mut UiBlend) -> bool {
    combo(ui, "Blend", blend, &UiBlend::ALL, blend_name)
}

/// A "Gradient" toggle and, when on, its kind, geometry and stops.
pub(super) fn gradient_editor(ui: &mut egui::Ui, gradient: &mut Option<UiGradient>) -> bool {
    let mut on = gradient.is_some();
    let toggled = ui.checkbox(&mut on, "Gradient").changed();
    if toggled {
        *gradient = on.then(UiGradient::default);
    }
    let Some(g) = gradient else {
        return toggled;
    };
    let kinds = [GradientKind::Linear, GradientKind::Radial];
    let mut changed = toggled | combo(ui, "Kind", &mut g.kind, &kinds, gradient_kind_name);
    match g.kind {
        GradientKind::Linear => changed |= drag_row(ui, "  Angle:", &mut g.angle, 1.0),
        GradientKind::Radial => {
            if let Some(c) = vec2_row(ui, "  Center:", g.center, 0.01) {
                g.center = c;
                changed = true;
            }
            changed |= drag_row(ui, "  Radius:", &mut g.radius, 0.01);
        }
    }
    changed | draw_stops(ui, &mut g.stops)
}

/// Each stop's position and colour, with add (up to four) and remove (down to two).
fn draw_stops(ui: &mut egui::Ui, stops: &mut Vec<GradientStop>) -> bool {
    let mut changed = false;
    let mut remove = None;
    for (i, s) in stops.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("  Stop {i}:"));
            changed |= ui
                .add(egui::DragValue::new(&mut s.t).speed(0.01).range(0.0..=1.0))
                .changed();
            let mut c = s.color.to_array();
            changed |= ui.color_edit_button_rgba_unmultiplied(&mut c).changed();
            s.color = Vec4::from_array(c);
            if ui.small_button("−").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove.filter(|_| stops.len() > 2) {
        stops.remove(i);
        changed = true;
    }
    if stops.len() < MAX_GRADIENT_STOPS && ui.small_button("+ Stop").clicked() {
        let last = stops.last().map_or(Vec4::ONE, |s| s.color);
        stops.push(GradientStop {
            t: 1.0,
            color: last,
        });
        changed = true;
    }
    changed
}

/// A labelled `x, y` row writing into `v` (`true` when it changed).
pub(super) fn vec2_edit(ui: &mut egui::Ui, label: &str, v: &mut Vec2, speed: f32) -> bool {
    match vec2_row(ui, label, *v, speed) {
        Some(n) => {
            *v = n;
            true
        }
        None => false,
    }
}
