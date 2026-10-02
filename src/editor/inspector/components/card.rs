//! Unity-style component card: the shared wrapper every inspector component
//! renders inside. A themed frame with a collapsible foldout header (type glyph +
//! title), an optional remove button, and a body. Collapse state is keyed by title
//! so a foldout stays open/closed per component type across selection changes.

use egui::collapsing_header::CollapsingState;
use egui_phosphor::regular as icon;

use crate::editor::theme::{self, chrome};

/// Render a component foldout card. Pass `Some(&mut flag)` for a removable
/// component (the X button sets the flag); pass `None` for mandatory ones
/// (e.g. Transform).
pub fn component_card(
    ui: &mut egui::Ui,
    glyph: &str,
    title: &str,
    remove: Option<&mut bool>,
    body: impl FnOnce(&mut egui::Ui),
) {
    let t = theme::from_ui(ui);
    card_frame(&t).show(ui, |ui| {
        let id = egui::Id::new(("component_card", title));
        CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                ui.colored_label(t.accent, glyph);
                ui.label(egui::RichText::new(title).font(theme::fonts::semibold(12.5)));
                if let Some(remove) = remove {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if chrome::icon_button(ui, icon::X, "Remove component").clicked() {
                            *remove = true;
                        }
                    });
                }
            })
            .body(body);
    });
    ui.add_space(t.space_xs);
}

/// The raised frame every inspector card sits in (components, the object header,
/// the asset cards): one surface step above the panel, softly rounded.
pub fn card_frame(t: &theme::Theme) -> egui::Frame {
    egui::Frame::none()
        .fill(t.bg_tier2)
        .inner_margin(egui::Margin::symmetric(t.space_sm, 6.0))
        .rounding(6.0)
        .stroke(egui::Stroke::new(1.0, t.outline.linear_multiply(0.6)))
}
