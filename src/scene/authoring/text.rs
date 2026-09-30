//! src/scene/authoring/text.rs — Shared Text-authoring ops (#419).
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `TextComponent` field by field. The editor's Text card and the Lua `Text.*`
//! namespace both route every write through these, so validation lives once:
//! colours stay in `[0, 1]`, sizes stay positive (auto-size keeps `min ≤ max`),
//! effect widths are non-negative, and an empty font path means "the default".
//!
//! Allowed deps: components (the `TextComponent` data). Pure.

use glam::{Vec2, Vec4};

use crate::components::{TextAlignment, TextComponent, TextOverflow};

/// The smallest size a text can be set to, in reference units.
pub const MIN_FONT_SIZE: f32 = 0.5;

/// Set the string drawn.
pub fn set_text(t: &mut TextComponent, text: String) {
    t.text = text;
}

/// The three font slots a text names: regular, bold and italic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSlot {
    Regular,
    Bold,
    Italic,
}

/// Set a font path; `None` or an empty path falls back (the default font for
/// `Regular`, a synthesized style for `Bold` / `Italic`).
pub fn set_font(t: &mut TextComponent, slot: FontSlot, path: Option<String>) {
    let path = path.filter(|p| !p.is_empty());
    match slot {
        FontSlot::Regular => t.font = path,
        FontSlot::Bold => t.font_bold = path,
        FontSlot::Italic => t.font_italic = path,
    }
}

/// Set the em size in reference units, kept ≥ [`MIN_FONT_SIZE`].
pub fn set_font_size(t: &mut TextComponent, size: f32) {
    t.font_size = positive(size);
}

/// Set the fill colour, each channel clamped to `[0, 1]`.
pub fn set_color(t: &mut TextComponent, color: Vec4) {
    t.color = unit(color);
}

/// Set where the block sits in the rect.
pub fn set_alignment(t: &mut TextComponent, alignment: TextAlignment) {
    t.alignment = alignment;
}

/// Set word wrapping.
pub fn set_wrap(t: &mut TextComponent, wrap: bool) {
    t.wrap = wrap;
}

/// Set what happens to text that does not fit.
pub fn set_overflow(t: &mut TextComponent, overflow: TextOverflow) {
    t.overflow = overflow;
}

/// Set the line-pitch multiplier, kept ≥ 0.
pub fn set_line_spacing(t: &mut TextComponent, spacing: f32) {
    t.line_spacing = spacing.max(0.0);
}

/// Set the extra advance between characters, in ems (negative tightens).
pub fn set_letter_spacing(t: &mut TextComponent, spacing: f32) {
    t.letter_spacing = spacing;
}

/// Set auto-size and its size range; both bounds stay positive and `min ≤ max`.
pub fn set_auto_size(t: &mut TextComponent, enabled: bool, min: f32, max: f32) {
    let (min, max) = (positive(min), positive(max));
    t.auto_size = enabled;
    t.auto_size_min = min.min(max);
    t.auto_size_max = max.max(min);
}

/// Set whether rich-text tags are parsed.
pub fn set_rich_text(t: &mut TextComponent, rich: bool) {
    t.rich_text = rich;
}

/// Set whether the pointer can hit this graphic.
pub fn set_raycast_target(t: &mut TextComponent, target: bool) {
    t.raycast_target = target;
}

/// Set the outline: thickness in ems (≥ 0) and colour.
pub fn set_outline(t: &mut TextComponent, width: f32, color: Vec4) {
    t.outline_width = width.max(0.0);
    t.outline_color = unit(color);
}

/// Set the drop shadow: offset in reference units and colour.
pub fn set_shadow(t: &mut TextComponent, offset: Vec2, color: Vec4) {
    t.shadow_offset = offset;
    t.shadow_color = unit(color);
}

/// Set the glow: reach in ems (≥ 0) and colour.
pub fn set_glow(t: &mut TextComponent, size: f32, color: Vec4) {
    t.glow_size = size.max(0.0);
    t.glow_color = unit(color);
}

fn positive(size: f32) -> f32 {
    if size.is_finite() {
        size.max(MIN_FONT_SIZE)
    } else {
        MIN_FONT_SIZE
    }
}

fn unit(c: Vec4) -> Vec4 {
    c.clamp(Vec4::ZERO, Vec4::ONE)
}

/// Parse an alignment name (case-insensitive), e.g. `"MiddleCenter"`.
pub fn parse_alignment(name: &str) -> Option<TextAlignment> {
    TextAlignment::ALL
        .into_iter()
        .find(|a| alignment_name(*a).eq_ignore_ascii_case(name))
}

/// The alignment's Unity name.
pub fn alignment_name(a: TextAlignment) -> &'static str {
    match a {
        TextAlignment::TopLeft => "TopLeft",
        TextAlignment::TopCenter => "TopCenter",
        TextAlignment::TopRight => "TopRight",
        TextAlignment::MiddleLeft => "MiddleLeft",
        TextAlignment::MiddleCenter => "MiddleCenter",
        TextAlignment::MiddleRight => "MiddleRight",
        TextAlignment::BottomLeft => "BottomLeft",
        TextAlignment::BottomCenter => "BottomCenter",
        TextAlignment::BottomRight => "BottomRight",
    }
}

/// Every overflow mode.
pub const OVERFLOWS: [TextOverflow; 3] = [
    TextOverflow::Overflow,
    TextOverflow::Truncate,
    TextOverflow::Ellipsis,
];

/// Parse an overflow name (case-insensitive).
pub fn parse_overflow(name: &str) -> Option<TextOverflow> {
    OVERFLOWS
        .into_iter()
        .find(|o| overflow_name(*o).eq_ignore_ascii_case(name))
}

/// The overflow mode's name.
pub fn overflow_name(o: TextOverflow) -> &'static str {
    match o {
        TextOverflow::Overflow => "Overflow",
        TextOverflow::Truncate => "Truncate",
        TextOverflow::Ellipsis => "Ellipsis",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ops_clamp_and_normalize() {
        let mut t = TextComponent::default();
        set_font_size(&mut t, -4.0);
        assert_eq!(t.font_size, MIN_FONT_SIZE);
        set_font_size(&mut t, f32::NAN);
        assert_eq!(t.font_size, MIN_FONT_SIZE);
        set_color(&mut t, Vec4::new(2.0, -1.0, 0.5, 0.25));
        assert_eq!(t.color, Vec4::new(1.0, 0.0, 0.5, 0.25));
        set_font(&mut t, FontSlot::Regular, Some(String::new()));
        assert_eq!(t.font, None);
        set_font(&mut t, FontSlot::Bold, Some("fonts/b.ttf".into()));
        assert_eq!(t.font_bold.as_deref(), Some("fonts/b.ttf"));
        set_outline(&mut t, -1.0, Vec4::ONE);
        assert_eq!(t.outline_width, 0.0);
        set_glow(&mut t, 0.2, Vec4::splat(3.0));
        assert_eq!((t.glow_size, t.glow_color), (0.2, Vec4::ONE));
    }

    #[test]
    fn auto_size_keeps_its_range_ordered() {
        let mut t = TextComponent::default();
        set_auto_size(&mut t, true, 40.0, 12.0);
        assert!(t.auto_size);
        assert_eq!((t.auto_size_min, t.auto_size_max), (12.0, 40.0));
    }

    #[test]
    fn names_round_trip() {
        for a in TextAlignment::ALL {
            assert_eq!(parse_alignment(alignment_name(a)), Some(a));
        }
        for o in OVERFLOWS {
            assert_eq!(parse_overflow(overflow_name(o)), Some(o));
        }
        assert_eq!(
            parse_alignment("middlecenter"),
            Some(TextAlignment::MiddleCenter)
        );
        assert_eq!(parse_overflow("Scroll"), None);
    }
}
