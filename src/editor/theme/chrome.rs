//! Shared panel chrome (#668): the frames, title strips, tabs and icon buttons
//! every docked panel is built from, so the hierarchy, inspector, bottom panel and
//! viewport read as one editor.

use egui::{Frame, Margin, Response, RichText, Stroke, Ui};

use super::{fonts, Theme};

/// Size of a panel title (Unity's tab labels: small and SemiBold, not a heading).
const TITLE_SIZE: f32 = 12.5;

/// The frame of a docked panel: panel fill, edged by the dark gutter.
pub fn panel_frame(t: &Theme) -> Frame {
    Frame::none()
        .fill(t.bg_tier1)
        .inner_margin(Margin::symmetric(t.space_sm, t.space_xs + 2.0))
        .stroke(Stroke::new(1.0, t.border))
}

/// The frame of a collapsed panel's rail.
pub fn rail_frame(t: &Theme) -> Frame {
    Frame::none()
        .fill(t.bg_tier1)
        .inner_margin(t.space_xs)
        .stroke(Stroke::new(1.0, t.border))
}

/// A panel's title strip: glyph + SemiBold title, a frameless `caret` button on
/// the right, then a hairline. Returns whether the caret was clicked.
pub fn panel_header(ui: &mut Ui, t: &Theme, glyph: &str, title: &str, caret: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(glyph).color(t.text_secondary));
        ui.label(RichText::new(title).font(fonts::semibold(ui.ctx(), TITLE_SIZE)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            clicked = icon_button(ui, caret, "Collapse").clicked();
        });
    });
    hairline(ui, t);
    clicked
}

/// A full-width 1px rule in the outline colour, tighter than egui's separator.
pub fn hairline(ui: &mut Ui, t: &Theme) {
    let rect = ui.available_rect_before_wrap();
    let y = ui.cursor().top() + 1.0;
    ui.painter()
        .hline(rect.x_range(), y, Stroke::new(1.0, t.outline));
    ui.add_space(t.space_xs);
}

/// A frameless glyph button with a tooltip: the collapse carets and per-panel tools.
pub fn icon_button(ui: &mut Ui, glyph: &str, hint: &str) -> Response {
    let t = super::from_ui(ui);
    ui.add(egui::Button::new(RichText::new(glyph).color(t.text_secondary)).frame(false))
        .on_hover_text(hint)
}

/// A Unity-6 style tab: plain text, and an accent underline when selected.
pub fn tab(ui: &mut Ui, t: &Theme, selected: bool, text: &str) -> Response {
    let color = if selected {
        t.text_primary
    } else {
        t.text_secondary
    };
    let resp = ui.add(egui::Button::new(RichText::new(text).color(color)).frame(false));
    if selected {
        let r = resp.rect;
        let y = r.bottom() + 2.0;
        ui.painter()
            .hline(r.x_range(), y, Stroke::new(2.0, t.accent));
    } else if resp.hovered() {
        let r = resp.rect;
        let y = r.bottom() + 2.0;
        ui.painter()
            .hline(r.x_range(), y, Stroke::new(2.0, t.outline));
    }
    resp
}
