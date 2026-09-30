//! src/editor/inspector/components/ribbons/ — the Trail and Line cards (#441).
//!
//! Both cards end in the same Style section (width curve, colour gradient,
//! texture, texture mode, blend) over the `RibbonStyle` they share. THIN clients
//! (#287): every widget reads a snapshot and routes its write through the shared
//! `authoring::trail` / `authoring::line` / `authoring::ribbon` ops the `Trail.*`
//! and `Line.*` Lua verbs call.

mod line;
mod trail;

pub use line::draw_line;
pub use trail::draw_trail;

use super::particles::widgets::{combo, curve_editor, gradient_editor};
use crate::components::{ParticleBlend, RibbonStyle, TextureMode};
use crate::core::curve::{Curve, Gradient};
use crate::scene::authoring::ribbon as ribbon_ops;

/// One edit the Style section made, applied through the shared ribbon ops.
enum StyleEdit {
    Width(Curve),
    Color(Gradient),
    Texture(String),
    Mode(TextureMode),
    Blend(ParticleBlend),
}

impl StyleEdit {
    fn apply(self, s: &mut RibbonStyle) {
        match self {
            Self::Width(c) => ribbon_ops::set_width(s, c),
            Self::Color(g) => ribbon_ops::set_color(s, g),
            Self::Texture(t) => ribbon_ops::set_texture(s, t),
            Self::Mode(m) => ribbon_ops::set_texture_mode(s, m),
            Self::Blend(b) => ribbon_ops::set_blend(s, b),
        }
    }
}

/// The Style section; returns the edit made this frame, if any.
fn draw_style(ui: &mut egui::Ui, s: &RibbonStyle) -> Option<StyleEdit> {
    ui.separator();
    ui.label("Style (0 = start, 1 = end)");
    let mut edit = None;
    if let Some(c) = curve_editor(ui, "Width", &s.width) {
        edit = Some(StyleEdit::Width(c));
    }
    if let Some(g) = gradient_editor(ui, "Color", &s.color) {
        edit = Some(StyleEdit::Color(g));
    }
    let mut tex = s.texture.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Texture:");
        if ui.text_edit_singleline(&mut tex).changed() {
            edit = Some(StyleEdit::Texture(tex.clone()));
        }
    });
    let mut mode = s.texture_mode;
    let modes = [
        (TextureMode::Stretch, "Stretch"),
        (TextureMode::Tile, "Tile"),
    ];
    if combo(ui, "Texture Mode", &mut mode, &modes) {
        edit = Some(StyleEdit::Mode(mode));
    }
    let mut blend = s.blend;
    let blends = [
        (ParticleBlend::Alpha, "Alpha"),
        (ParticleBlend::Additive, "Additive"),
    ];
    if combo(ui, "Blend", &mut blend, &blends) {
        edit = Some(StyleEdit::Blend(blend));
    }
    edit
}
