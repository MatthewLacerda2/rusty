use glam::{Vec2, Vec4};

use rusty_components::{TextAlignment, TextComponent, TextOverflow};
use rusty_scene::rules::sanitize::{at_least, finite, finite2, non_negative, unit_color};

pub const MIN_FONT_SIZE: f32 = 0.5;

pub fn set_text(t: &mut TextComponent, text: String) {
    t.text = text;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontSlot {
    Regular,
    Bold,
    Italic,
}

/// `None` or an empty path falls back: the default font for `Regular`, a synthesized style for
/// `Bold` / `Italic`.
pub fn set_font(t: &mut TextComponent, slot: FontSlot, path: Option<String>) {
    let path = path.filter(|p| !p.is_empty());
    match slot {
        FontSlot::Regular => t.font = path,
        FontSlot::Bold => t.font_bold = path,
        FontSlot::Italic => t.font_italic = path,
    }
}

pub fn set_font_size(t: &mut TextComponent, size: f32) {
    t.font_size = at_least(size, MIN_FONT_SIZE);
}

pub fn set_color(t: &mut TextComponent, color: Vec4) {
    t.color = unit_color(color);
}

pub fn set_alignment(t: &mut TextComponent, alignment: TextAlignment) {
    t.alignment = alignment;
}

pub fn set_wrap(t: &mut TextComponent, wrap: bool) {
    t.wrap = wrap;
}

pub fn set_overflow(t: &mut TextComponent, overflow: TextOverflow) {
    t.overflow = overflow;
}

pub fn set_line_spacing(t: &mut TextComponent, spacing: f32) {
    t.line_spacing = non_negative(spacing);
}

pub fn set_letter_spacing(t: &mut TextComponent, spacing: f32) {
    t.letter_spacing = finite(spacing);
}

pub fn set_auto_size(t: &mut TextComponent, enabled: bool, min: f32, max: f32) {
    let (min, max) = (at_least(min, MIN_FONT_SIZE), at_least(max, MIN_FONT_SIZE));
    t.auto_size = enabled;
    t.auto_size_min = min.min(max);
    t.auto_size_max = max.max(min);
}

pub fn set_rich_text(t: &mut TextComponent, rich: bool) {
    t.rich_text = rich;
}

pub fn set_raycast_target(t: &mut TextComponent, target: bool) {
    t.raycast_target = target;
}

pub fn set_blend(t: &mut TextComponent, blend: rusty_components::UiBlend) {
    t.blend = blend;
}

pub fn set_outline(t: &mut TextComponent, width: f32, color: Vec4) {
    t.outline_width = non_negative(width);
    t.outline_color = unit_color(color);
}

pub fn set_shadow(t: &mut TextComponent, offset: Vec2, color: Vec4) {
    t.shadow_offset = finite2(offset);
    t.shadow_color = unit_color(color);
}

pub fn set_glow(t: &mut TextComponent, size: f32, color: Vec4) {
    t.glow_size = non_negative(size);
    t.glow_color = unit_color(color);
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
}
