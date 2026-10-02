//! How the [`Theme`] tokens map onto egui's [`Style`]: the type scale, the
//! compact (Unity / JetBrains) density, and every widget state's colours. The one
//! place that configures egui — panels only read tokens.

use egui::epaint::Shadow;
use egui::style::{ScrollStyle, WidgetVisuals};
use egui::{Color32, FontFamily, FontId, Margin, Rounding, Stroke, Style, TextStyle};

use super::{fonts, Theme};

/// Corner radius of buttons, fields and rows.
const WIDGET_RADIUS: f32 = 4.0;
/// Corner radius of windows, menus and popups.
const WINDOW_RADIUS: f32 = 8.0;

pub(super) fn configure(t: &Theme, ctx: &egui::Context, style: &mut Style) {
    type_scale(ctx, style);
    density(t, style);
    visuals(t, style);
}

/// Inter at compact UI sizes; headings are SemiBold, not bigger.
fn type_scale(ctx: &egui::Context, style: &mut Style) {
    let inter = |size| FontId::new(size, FontFamily::Proportional);
    style.text_styles = [
        (TextStyle::Heading, fonts::semibold(ctx, 13.5)),
        (TextStyle::Body, inter(12.5)),
        (TextStyle::Button, inter(12.5)),
        (TextStyle::Small, inter(11.0)),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
    ]
    .into();
}

fn density(t: &Theme, style: &mut Style) {
    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(6.0, t.space_xs);
    s.button_padding = egui::vec2(6.0, 2.0);
    s.interact_size = egui::vec2(36.0, 20.0);
    s.indent = 14.0;
    s.icon_width = 13.0;
    s.icon_width_inner = 7.0;
    s.icon_spacing = 5.0;
    s.menu_margin = Margin::symmetric(t.space_xs, t.space_xs);
    s.window_margin = Margin::same(t.space_md);
    s.combo_height = 260.0;
    s.scroll = ScrollStyle::thin();
}

fn visuals(t: &Theme, style: &mut Style) {
    let v = &mut style.visuals;
    v.dark_mode = true;
    v.override_text_color = None;
    v.panel_fill = t.bg_tier1;
    v.window_fill = t.bg_tier2;
    v.window_stroke = Stroke::new(1.0, t.outline);
    v.window_rounding = Rounding::same(WINDOW_RADIUS);
    v.menu_rounding = Rounding::same(6.0);
    v.window_shadow = shadow(16.0, 110);
    v.popup_shadow = shadow(10.0, 90);
    v.extreme_bg_color = t.bg_tier0;
    v.faint_bg_color = t.bg_tier2;
    v.code_bg_color = t.bg_tier0;
    v.hyperlink_color = t.accent;
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.danger;
    v.text_cursor = Stroke::new(1.5, t.accent);
    v.slider_trailing_fill = true;
    v.indent_has_left_vline = true;
    v.collapsing_header_frame = false;

    // Selected rows and text: a dim blue fill, light-blue text and focus edge.
    v.selection.bg_fill = t.selection;
    v.selection.stroke = Stroke::new(1.0, Color32::from_rgb(196, 220, 255));

    let w = &mut v.widgets;
    // Labels, separators, frames.
    w.noninteractive = state(t.bg_tier1, t.outline, t.text_primary);
    // Buttons, fields and toggles at rest: a quiet raised fill, no loud edge.
    w.inactive = state(t.bg_hover, t.bg_hover, t.text_primary);
    w.inactive.bg_fill = t.bg_tier0;
    w.hovered = state(t.outline, t.outline, Color32::WHITE);
    w.hovered.bg_fill = t.bg_tier0;
    w.hovered.bg_stroke = Stroke::new(1.0, t.text_secondary);
    w.active = state(t.selection, t.accent, Color32::WHITE);
    w.open = state(t.bg_tier2, t.outline, t.text_primary);
}

/// One widget state: `fill` behind buttons (`weak_bg_fill`) and fields (`bg_fill`),
/// a 1px `edge`, and `text` for labels and glyphs.
fn state(fill: Color32, edge: Color32, text: Color32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, edge),
        rounding: Rounding::same(WIDGET_RADIUS),
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    }
}

fn shadow(blur: f32, alpha: u8) -> Shadow {
    Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur,
        spread: 0.0,
        color: Color32::from_black_alpha(alpha),
    }
}
