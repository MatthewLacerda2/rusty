//! Editor theme — the single source of truth for the editor's look.
//!
//! egui is immediate-mode: there is no CSS cascade, so "theming" is done by
//! configuring [`egui::Style`] / [`egui::Visuals`] / fonts once and having every
//! panel read the same tokens instead of hardcoding `Color32::from_rgb(...)`.
//!
//! [`Theme`] is the token set (colors + spacing + type scale). It is applied each
//! frame via [`Theme::apply`] and also stashed in the egui context memory so that
//! deeply-nested widgets which only receive a `&Ui` can still read tokens via
//! [`from_ctx`] / [`from_ui`] without threading the struct through every call.

use egui::{Color32, Context, FontFamily, FontId, Rounding, Stroke, TextStyle, Ui};

/// Brand palette + spacing/type scale. Cheap to clone (all `Copy` fields).
#[derive(Clone, Copy)]
pub struct Theme {
    // Background tiers (deepest → raised)
    pub bg_tier0: Color32,
    pub bg_tier1: Color32,
    pub bg_tier2: Color32,
    pub border: Color32,

    // Text
    pub text_primary: Color32,
    pub text_secondary: Color32,

    // Accents (used sparingly)
    pub accent_purple: Color32,
    pub accent_blue: Color32,
    pub accent_yellow: Color32,
    pub danger: Color32,
    /// Dim purple used as the selection fill.
    pub selection: Color32,

    // Spacing scale (4/8px grid)
    pub space_xs: f32,
    pub space_sm: f32,
    pub space_md: f32,
    pub space_lg: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// The default dark-grey / silver-grey identity with purple/blue/yellow accents.
    pub fn dark() -> Self {
        Self {
            bg_tier0: Color32::from_rgb(26, 26, 31),
            bg_tier1: Color32::from_rgb(34, 34, 40),
            bg_tier2: Color32::from_rgb(44, 44, 52),
            border: Color32::from_rgb(58, 58, 66),

            text_primary: Color32::from_rgb(220, 222, 228),
            text_secondary: Color32::from_rgb(150, 152, 160),

            accent_purple: Color32::from_rgb(150, 120, 230),
            accent_blue: Color32::from_rgb(80, 150, 240),
            accent_yellow: Color32::from_rgb(240, 200, 90),
            danger: Color32::from_rgb(235, 80, 110),
            selection: Color32::from_rgb(60, 52, 92),

            space_xs: 4.0,
            space_sm: 8.0,
            space_md: 12.0,
            space_lg: 16.0,
        }
    }

    /// Accent color for an asset of the given (lowercased) file extension, used by
    /// the content browser to color-badge tiles by type.
    pub fn asset_color(&self, ext: &str) -> Color32 {
        match ext {
            "png" | "tga" | "jpg" | "jpeg" => self.accent_blue,
            "wav" | "mp3" | "ogg" => self.accent_purple,
            "scene" => self.accent_yellow,
            "lua" => self.text_primary,
            _ => self.text_secondary,
        }
    }

    /// Register the Phosphor icon font (the `lucide-react` equivalent) so glyph
    /// constants from [`egui_phosphor::regular`] render inline with text. Call once.
    pub fn install_fonts(ctx: &Context) {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        ctx.set_fonts(fonts);
    }

    /// Apply colors, spacing and the type scale to the egui style, and publish the
    /// tokens into context memory for `&Ui`-only readers ([`from_ui`]).
    pub fn apply(&self, ctx: &Context) {
        let mut style = (*ctx.style()).clone();

        // Type scale.
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(17.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(13.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(12.5, FontFamily::Monospace),
            ),
        ]
        .into();

        // Spacing scale.
        style.spacing.item_spacing = egui::vec2(self.space_sm, self.space_xs + 2.0);
        style.spacing.button_padding = egui::vec2(self.space_sm, self.space_xs);

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = Some(self.text_primary);
        v.panel_fill = self.bg_tier1;
        v.window_fill = self.bg_tier1;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.extreme_bg_color = self.bg_tier0;
        v.window_rounding = Rounding::same(6.0);

        v.widgets.noninteractive.bg_fill = self.bg_tier1;
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.border);

        v.widgets.inactive.bg_fill = self.bg_tier2;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, self.text_secondary);
        v.widgets.inactive.rounding = Rounding::same(4.0);

        v.widgets.hovered.bg_fill = self.bg_tier2;
        v.widgets.hovered.fg_stroke = Stroke::new(1.2, self.accent_blue);
        v.widgets.hovered.rounding = Rounding::same(4.0);

        v.widgets.active.bg_fill = self.bg_tier2;
        v.widgets.active.fg_stroke = Stroke::new(1.5, self.accent_purple);
        v.widgets.active.rounding = Rounding::same(4.0);

        v.selection.bg_fill = self.selection;
        v.selection.stroke = Stroke::new(1.0, self.accent_purple);

        ctx.set_style(style);
        ctx.data_mut(|d| d.insert_temp(token_id(), *self));
    }
}

fn token_id() -> egui::Id {
    egui::Id::new("rusty_editor_theme")
}

/// Read the active theme tokens from the egui context (falls back to the default
/// dark theme if none has been applied yet this frame).
pub fn from_ctx(ctx: &Context) -> Theme {
    ctx.data(|d| d.get_temp::<Theme>(token_id()))
        .unwrap_or_default()
}

/// Convenience wrapper around [`from_ctx`] for widgets that only hold a `&Ui`.
pub fn from_ui(ui: &Ui) -> Theme {
    from_ctx(ui.ctx())
}
