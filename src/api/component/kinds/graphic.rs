//! src/api/component/kinds/graphic.rs — the drawn UI components' field tables:
//! `Text` and `Shape`. A setter taking several fields (`set_outline(width,
//! color)`) is one group, called once with each field's new or current value.

use glam::{Vec2, Vec4};

use super::super::table::{group, inspect, op, part, with, Group};
use super::set_ui_shader;
use crate::components::{ShapeComponent, ShapeGlow, ShapeShadow, TextComponent, UiShader};
use crate::scene::authoring::shape;
use crate::scene::authoring::text::{self, FontSlot};

const AUTO_SIZE: Group<TextComponent> = group!(
    |t, auto_size: bool, auto_size_min: f32, auto_size_max: f32| {
        text::set_auto_size(t, auto_size, auto_size_min, auto_size_max)
    }
);
const OUTLINE: Group<TextComponent> = group!(|t, outline_width: f32, outline_color: Vec4| {
    text::set_outline(t, outline_width, outline_color)
});
const TEXT_SHADOW: Group<TextComponent> = group!(|t, shadow_offset: Vec2, shadow_color: Vec4| {
    text::set_shadow(t, shadow_offset, shadow_color)
});
const TEXT_GLOW: Group<TextComponent> =
    group!(|t, glow_size: f32, glow_color: Vec4| text::set_glow(t, glow_size, glow_color));
const ARC: Group<ShapeComponent> =
    group!(|s, arc_start: f32, arc_end: f32| shape::set_arc(s, arc_start, arc_end));
const DASH: Group<ShapeComponent> = group!(|s, dash: f32, gap: f32| shape::set_dash(s, dash, gap));
const BORDER: Group<ShapeComponent> = group!(|s, border_width: f32, border_color: Vec4| {
    shape::set_border(s, border_width, border_color)
});

inspect! {
    TextComponent => text, text_mut, &[
        op!("text", text::set_text),
        with!("font", |t, v: Option<String>| text::set_font(t, FontSlot::Regular, v)),
        with!("font_bold", |t, v: Option<String>| text::set_font(t, FontSlot::Bold, v)),
        with!("font_italic", |t, v: Option<String>| text::set_font(t, FontSlot::Italic, v)),
        op!("font_size", text::set_font_size),
        op!("color", text::set_color),
        op!("alignment", text::set_alignment),
        op!("wrap", text::set_wrap),
        op!("overflow", text::set_overflow),
        op!("line_spacing", text::set_line_spacing),
        op!("letter_spacing", text::set_letter_spacing),
        part!("auto_size", AUTO_SIZE),
        part!("auto_size_min", AUTO_SIZE),
        part!("auto_size_max", AUTO_SIZE),
        op!("rich_text", text::set_rich_text),
        op!("raycast_target", text::set_raycast_target),
        part!("outline_width", OUTLINE),
        part!("outline_color", OUTLINE),
        part!("shadow_offset", TEXT_SHADOW),
        part!("shadow_color", TEXT_SHADOW),
        part!("glow_size", TEXT_GLOW),
        part!("glow_color", TEXT_GLOW),
        op!("blend", text::set_blend),
        with!("shader", |t, v: Option<UiShader>| set_ui_shader(&mut t.shader, v)?),
    ];
    ShapeComponent => shape, shape_mut, &[
        op!("kind", shape::set_kind),
        op!("corner", shape::set_corner),
        op!("radius", shape::set_radius),
        op!("inner_radius", shape::set_inner_radius),
        part!("arc_start", ARC),
        part!("arc_end", ARC),
        op!("thickness", shape::set_thickness),
        part!("dash", DASH),
        part!("gap", DASH),
        op!("color", shape::set_color),
        op!("gradient", shape::set_gradient),
        part!("border_width", BORDER),
        part!("border_color", BORDER),
        with!("shadow", |s, v: ShapeShadow| shape::set_shadow(s, v.offset, v.blur, v.color)),
        with!("glow", |s, v: ShapeGlow| shape::set_glow(s, v.size, v.intensity, v.color)),
        op!("blend", shape::set_blend),
        op!("raycast_target", shape::set_raycast_target),
        with!("shader", |s, v: Option<UiShader>| set_ui_shader(&mut s.shader, v)?),
    ];
}
