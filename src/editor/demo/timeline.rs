//! The timeline strip over the Game view: play/pause, the scrub bar with the
//! index's marks on it, the tick readout and a close button.

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Ui};
use egui_phosphor::regular as icon;

use super::DemoView;
use crate::editor::theme::Theme;

const HEIGHT: f32 = 26.0;
const MARK_HIT: f32 = 4.0;

/// The tick under a pointer at `x` on a bar spanning `left..right`, of `ticks`.
pub fn tick_at(x: f32, left: f32, right: f32, ticks: u64) -> u64 {
    if ticks == 0 || right <= left {
        return 0;
    }
    let last = ticks - 1;
    let t = ((x - left) / (right - left)).clamp(0.0, 1.0);
    ((t as f64 * last as f64).round() as u64).min(last)
}

/// Where `tick` sits on a bar spanning `left..right`, of `ticks`.
pub fn x_of(tick: u64, left: f32, right: f32, ticks: u64) -> f32 {
    let last = ticks.saturating_sub(1).max(1);
    left + (right - left) * (tick.min(last) as f32 / last as f32)
}

/// The mark within a few pixels of `x`, if any.
pub fn mark_near(
    marks: &[(u64, String)],
    x: f32,
    left: f32,
    right: f32,
    ticks: u64,
) -> Option<&str> {
    marks
        .iter()
        .find(|(tick, _)| (x_of(*tick, left, right, ticks) - x).abs() <= MARK_HIT)
        .map(|(_, label)| label.as_str())
}

/// Draw the strip and record what the pointer asked for in `view`.
pub fn draw(ui: &mut Ui, view: &mut DemoView, t: &Theme) {
    ui.horizontal(|ui| {
        ui.set_height(HEIGHT);
        let glyph = if view.paused { icon::PLAY } else { icon::PAUSE };
        let hint = if view.paused {
            "Play the demo"
        } else {
            "Pause the demo"
        };
        if ui
            .button(RichText::new(glyph).color(t.text_primary))
            .on_hover_text(hint)
            .clicked()
        {
            // Play at the end starts over, as a media player does.
            if view.paused && view.at_end() {
                view.seek = Some(0);
            }
            view.paused = !view.paused;
        }
        let readout = format!("{} / {}", view.current, view.ticks.saturating_sub(1));
        let reserve = 90.0 + ui.spacing().item_spacing.x * 2.0 + 28.0;
        let width = (ui.available_width() - reserve).max(40.0);
        bar(ui, view, t, width);
        ui.label(RichText::new(readout).color(t.text_secondary).monospace());
        if ui.button(icon::X).on_hover_text("Close the demo").clicked() {
            view.close = true;
        }
    });
}

/// The scrub bar: a click or drag asks for the tick under the pointer.
fn bar(ui: &mut Ui, view: &mut DemoView, t: &Theme, width: f32) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, HEIGHT), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 6.0));
    let painter = ui.painter_at(rect);
    painter.rect_filled(track, 3.0, t.bg_tier0);
    let head = x_of(view.current, rect.left(), rect.right(), view.ticks);
    painter.rect_filled(
        Rect::from_min_max(track.min, egui::pos2(head, track.max.y)),
        3.0,
        t.accent,
    );
    for (tick, _) in &view.marks {
        let x = x_of(*tick, rect.left(), rect.right(), view.ticks);
        painter.line_segment(
            [
                egui::pos2(x, rect.top() + 4.0),
                egui::pos2(x, rect.bottom() - 4.0),
            ],
            Stroke::new(1.5, t.warning),
        );
    }
    painter.circle_filled(egui::pos2(head, rect.center().y), 5.0, Color32::WHITE);
    if let Some(pos) = response.hover_pos() {
        if let Some(label) = mark_near(&view.marks, pos.x, rect.left(), rect.right(), view.ticks) {
            painter.text(
                egui::pos2(pos.x, rect.top()),
                Align2::CENTER_BOTTOM,
                label,
                FontId::proportional(11.0),
                t.text_primary,
            );
        }
    }
    if let (true, Some(pos)) = (
        response.is_pointer_button_down_on(),
        response.interact_pointer_pos(),
    ) {
        let tick = tick_at(pos.x, rect.left(), rect.right(), view.ticks);
        if tick != view.current {
            view.seek = Some(tick);
        }
    }
}

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;
