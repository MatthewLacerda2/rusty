//! The RectTransform card's anchor-preset picker (#423): Unity's 4×4 grid in a
//! popup. A click re-anchors without moving the element; held **Shift** also
//! sets the pivot, held **Alt** also snaps the element onto its anchors. The cell
//! is applied through `scene::authoring::rect_transform::apply_anchor_preset`, the
//! op `RectTransform.SetAnchorPreset` calls.

use crate::components::RectTransformComponent;
use crate::scene::authoring::rect_transform::{AnchorPreset, AxisPreset};
use egui::{Rect, Sense, Stroke, StrokeKind};

/// Rows top to bottom, then stretch — Unity's order.
const ROWS: [AxisPreset; 4] = [
    AxisPreset::Max,
    AxisPreset::Center,
    AxisPreset::Min,
    AxisPreset::Stretch,
];
const CELL: f32 = 30.0;

/// The preset button and its popup over the snapshot `r`. Returns the preset the
/// user picked this frame.
pub fn picker(ui: &mut egui::Ui, r: &RectTransformComponent) -> Option<AnchorPreset> {
    let current = (
        AxisPreset::of(r.anchor_min.x, r.anchor_max.x),
        AxisPreset::of(r.anchor_min.y, r.anchor_max.y),
    );
    let label = match current {
        (Some(x), Some(y)) => format!("{} / {}", x.name(0), y.name(1)),
        _ => "custom".to_string(),
    };
    let button = ui.horizontal(|ui| {
        ui.label("Anchors:");
        ui.button(label)
    });
    let button = button.inner;
    let mut picked = None;
    // As egui 0.27's `popup_below_widget`: button-wide, justified, and any click
    // (a picked cell included) closes it.
    egui::Popup::from_toggle_button_response(&button)
        .id(ui.make_persistent_id("rusty.anchor_presets"))
        .layout(egui::Layout::top_down_justified(egui::Align::LEFT))
        .width(button.rect.width())
        .show(|ui| {
            ui.label("Shift: also set pivot · Alt: also set position");
            egui::Grid::new("anchor_preset_grid")
                .spacing([4.0, 4.0])
                .show(ui, |ui| {
                    for y in ROWS {
                        for x in AxisPreset::ALL {
                            let on = current == (Some(x), Some(y));
                            if cell(ui, x, y, on).clicked() {
                                let m = ui.input(|i| i.modifiers);
                                picked = Some(AnchorPreset {
                                    x,
                                    y,
                                    set_pivot: m.shift,
                                    set_position: m.alt,
                                });
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    picked
}

/// One grid cell: the parent box with the anchor drawn in it — a dot for a point
/// anchor, a bar across for a stretched axis.
fn cell(ui: &mut egui::Ui, x: AxisPreset, y: AxisPreset, on: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(CELL, CELL), Sense::click());
    let t = crate::editor::theme::from_ui(ui);
    let fill = if on || response.hovered() {
        t.bg_tier2
    } else {
        t.bg_tier1
    };
    let p = ui.painter();
    p.rect_filled(rect, 2.0, fill);
    let inner = rect.shrink(7.0);
    p.rect_stroke(
        inner,
        0.0,
        Stroke::new(1.0, t.text_secondary),
        StrokeKind::Middle,
    );
    let colour = if on { t.accent } else { t.danger };
    let span = |a: AxisPreset| match a {
        AxisPreset::Min => (0.0, 0.0),
        AxisPreset::Center => (0.5, 0.5),
        AxisPreset::Max => (1.0, 1.0),
        AxisPreset::Stretch => (-0.2, 1.2),
    };
    let ((x0, x1), (y0, y1)) = (span(x), span(y));
    // y-up presets onto the y-down painter.
    let at = |u: f32, v: f32| inner.lerp_inside(egui::vec2(u, 1.0 - v));
    let bar = Rect::from_two_pos(at(x0, y0), at(x1, y1)).expand(1.5);
    p.rect_filled(bar, 0.0, colour);
    response
}
