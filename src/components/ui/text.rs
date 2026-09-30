//! src/components/ui/text.rs — Text: an SDF-rendered UI label (#419).
//!
//! Unity's TextMeshPro `TextMeshProUGUI`, trimmed to what a HUD and menus need. It
//! draws `text` inside its entity's laid-out rect with a font asset (a `.ttf` /
//! `.otf` path, like textures; `None` is the bundled default), sized in canvas
//! reference units. Layout — measuring, wrapping, overflow, auto-size — is pure CPU
//! (`ui::text`), so a headless run lays text out exactly as the window does; the
//! renderer draws it from a signed-distance-field atlas, which keeps it sharp at
//! any size and makes the outline, drop shadow and glow nearly free.
//!
//! With `rich_text` on, a small tag subset styles runs inline: `<color=#rrggbb>` /
//! `<color=#rrggbbaa>`, `<b>`, `<i>` and `<size=n>`. Colours are display-space
//! (sRGB-encoded) with straight alpha, as a designer picks them. Pure authoring data.

use glam::{Vec2, Vec4};
use serde::{Deserialize, Serialize};

/// Where the text block sits in its rect — Unity's nine `TextAnchor`s.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignment {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl TextAlignment {
    /// Every alignment, in Unity's order (rows top to bottom, left to right).
    pub const ALL: [TextAlignment; 9] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::MiddleLeft,
        Self::MiddleCenter,
        Self::MiddleRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    /// The block's placement as fractions of the free space: `x` 0 left … 1 right,
    /// `y` 0 bottom … 1 top.
    pub fn fractions(self) -> Vec2 {
        let i = Self::ALL.iter().position(|a| *a == self).unwrap_or(0);
        Vec2::new((i % 3) as f32 * 0.5, 1.0 - (i / 3) as f32 * 0.5)
    }
}

/// What happens to text that does not fit its rect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextOverflow {
    /// Draw it anyway, spilling past the rect.
    #[default]
    Overflow,
    /// Drop the lines (and, unwrapped, the characters) that do not fit.
    Truncate,
    /// As `Truncate`, ending the last visible line with `…`.
    Ellipsis,
}

/// A UI text label. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextComponent {
    /// The string drawn (`\n` breaks a line; rich-text tags when `rich_text`).
    pub text: String,
    /// The font asset's path; `None` uses the bundled default (Instrument Sans).
    pub font: Option<String>,
    /// A bold face used by `<b>`; without one bold is synthesized (SDF dilation).
    pub font_bold: Option<String>,
    /// An italic face used by `<i>`; without one italic is synthesized (shear).
    pub font_italic: Option<String>,
    /// The em size in reference units.
    pub font_size: f32,
    /// RGBA fill, display space, straight alpha, each channel in `[0, 1]`.
    pub color: Vec4,
    /// Where the text block sits in the rect.
    pub alignment: TextAlignment,
    /// Break lines at word boundaries to fit the rect's width.
    pub wrap: bool,
    /// What happens to text that does not fit.
    pub overflow: TextOverflow,
    /// Line pitch multiplier (1 is the font's own line height).
    pub line_spacing: f32,
    /// Extra advance between characters, in ems (0.1 = a tenth of the font size).
    pub letter_spacing: f32,
    /// Pick the largest size in `[auto_size_min, auto_size_max]` that fits the rect
    /// (`font_size` then only scales `<size>` tags).
    pub auto_size: bool,
    /// Auto-size's smallest size, reference units.
    pub auto_size_min: f32,
    /// Auto-size's largest size, reference units.
    pub auto_size_max: f32,
    /// Parse the rich-text tag subset (off draws tags literally, e.g. for user input).
    pub rich_text: bool,
    /// Whether the pointer can hit this graphic (read by #420).
    pub raycast_target: bool,
    /// Outline thickness in ems (0 = none), drawn outside the glyph edge.
    pub outline_width: f32,
    /// Outline colour, display space, straight alpha.
    pub outline_color: Vec4,
    /// Drop-shadow offset in reference units (`x` right, `y` up); zero = none.
    pub shadow_offset: Vec2,
    /// Drop-shadow colour, display space, straight alpha.
    pub shadow_color: Vec4,
    /// Glow reach past the (outlined) edge in ems (0 = none) — the neon look.
    pub glow_size: f32,
    /// Glow colour, display space, straight alpha.
    pub glow_color: Vec4,
    /// Runtime only, never saved: the colour a `Selectable`'s `ColorTint` multiplies
    /// into every text colour (Unity's `CanvasRenderer` colour). Written by the event
    /// system (#420).
    #[serde(skip)]
    pub state_tint: Vec4,
}

impl Default for TextComponent {
    fn default() -> Self {
        Self {
            text: "New Text".into(),
            font: None,
            font_bold: None,
            font_italic: None,
            font_size: 36.0,
            color: Vec4::new(0.196, 0.196, 0.196, 1.0),
            alignment: TextAlignment::TopLeft,
            wrap: true,
            overflow: TextOverflow::Overflow,
            line_spacing: 1.0,
            letter_spacing: 0.0,
            auto_size: false,
            auto_size_min: 10.0,
            auto_size_max: 72.0,
            rich_text: true,
            raycast_target: true,
            outline_width: 0.0,
            outline_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
            shadow_offset: Vec2::ZERO,
            shadow_color: Vec4::new(0.0, 0.0, 0.0, 0.5),
            glow_size: 0.0,
            glow_color: Vec4::new(0.0, 1.0, 1.0, 0.75),
            state_tint: Vec4::ONE,
        }
    }
}
