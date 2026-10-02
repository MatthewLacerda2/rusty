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
//!
//! The look (#668): Unity 6's layout and compact density, finished with a cool
//! blue accent, Inter / JetBrains Mono ([`fonts`]) and the shared panel chrome in
//! [`chrome`].

pub mod chrome;
pub mod fonts;
mod style;

use egui::{Color32, Context, Ui};

/// Brand palette + spacing/type scale. Cheap to clone (all `Copy` fields).
#[derive(Clone, Copy)]
pub struct Theme {
    // Background tiers (deepest → raised)
    /// Wells: the viewport backdrop, text fields, the console.
    pub bg_tier0: Color32,
    /// Panels.
    pub bg_tier1: Color32,
    /// Raised surfaces: cards, panel title strips, the toolbar.
    pub bg_tier2: Color32,
    /// A hovered row or control.
    pub bg_hover: Color32,
    /// The dark gutter between panels.
    pub border: Color32,
    /// The lighter edge of controls, cards and separators inside a panel.
    pub outline: Color32,

    // Text
    pub text_primary: Color32,
    pub text_secondary: Color32,

    // Accents (used sparingly)
    /// The brand blue: selection edges, active toggles, focus, the play tint.
    pub accent: Color32,
    /// Dim blue used as the selection fill.
    pub selection: Color32,
    /// Warnings and dirty state.
    pub warning: Color32,
    pub danger: Color32,
    /// A second hue for type badges (audio), so the blue keeps meaning "selected".
    pub teal: Color32,

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
    /// The one theme: neutral, slightly cool greys with a bright azure accent —
    /// cooler and brighter than Unity's muted selection blue, so the two don't
    /// read as the same product.
    pub fn dark() -> Self {
        Self {
            bg_tier0: Color32::from_rgb(22, 23, 26),
            bg_tier1: Color32::from_rgb(33, 34, 38),
            bg_tier2: Color32::from_rgb(41, 43, 47),
            bg_hover: Color32::from_rgb(52, 55, 61),
            border: Color32::from_rgb(15, 16, 18),
            outline: Color32::from_rgb(56, 59, 66),

            text_primary: Color32::from_rgb(223, 226, 231),
            text_secondary: Color32::from_rgb(146, 151, 161),

            accent: Color32::from_rgb(58, 142, 255),
            selection: Color32::from_rgb(31, 68, 124),
            warning: Color32::from_rgb(240, 196, 84),
            danger: Color32::from_rgb(236, 84, 100),
            teal: Color32::from_rgb(70, 196, 178),

            space_xs: 4.0,
            space_sm: 8.0,
            space_md: 12.0,
            space_lg: 16.0,
        }
    }

    /// This theme while the game is playing: every surface leans toward the accent,
    /// Unity's cue that edits made now are lost on Stop.
    pub fn for_play_mode(self) -> Self {
        let tint = |c: Color32| lerp(c, self.accent, 0.10);
        Self {
            bg_tier0: tint(self.bg_tier0),
            bg_tier1: tint(self.bg_tier1),
            bg_tier2: tint(self.bg_tier2),
            bg_hover: tint(self.bg_hover),
            ..self
        }
    }

    /// Accent color for an asset of the given (lowercased) file extension, used by
    /// the content browser to color-badge tiles by type.
    pub fn asset_color(&self, ext: &str) -> Color32 {
        match ext {
            "png" | "tga" | "jpg" | "jpeg" => self.accent,
            "wav" | "mp3" | "ogg" => self.teal,
            "scene" => self.warning,
            "lua" => self.text_primary,
            _ => self.text_secondary,
        }
    }

    /// Register the editor's fonts (Inter, JetBrains Mono, Phosphor icons). Call once.
    pub fn install_fonts(ctx: &Context) {
        fonts::install(ctx);
    }

    /// Apply colors, spacing and the type scale to the egui style, and publish the
    /// tokens into context memory for `&Ui`-only readers ([`from_ui`]).
    pub fn apply(&self, ctx: &Context) {
        let mut s = (*ctx.style()).clone();
        style::configure(self, &mut s);
        ctx.set_style(s);
        ctx.data_mut(|d| d.insert_temp(token_id(), *self));
    }
}

/// Linear blend from `a` toward `b` by `t` (0 = `a`, 1 = `b`), alpha kept from `a`.
fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        mix(a.r(), b.r()),
        mix(a.g(), b.g()),
        mix(a.b(), b.b()),
        a.a(),
    )
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
