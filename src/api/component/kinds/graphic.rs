//! src/api/component/kinds/graphic.rs — the drawn UI components' field tables:
//! `Text` and `Shape`. Paired setters (`set_outline(width, color)`) are split per
//! field, the other half kept as it is.

use glam::{Vec2, Vec4};

use super::super::table::{inspect, op, with};
use super::set_ui_shader;
use crate::components::{ShapeComponent, ShapeGlow, ShapeShadow, TextComponent, UiShader};
use crate::scene::authoring::shape;
use crate::scene::authoring::text::{self, FontSlot};

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
        with!("auto_size", |t, v: bool| {
            text::set_auto_size(t, v, t.auto_size_min, t.auto_size_max)
        }),
        with!("auto_size_min", |t, v: f32| {
            text::set_auto_size(t, t.auto_size, v, t.auto_size_max)
        }),
        with!("auto_size_max", |t, v: f32| {
            text::set_auto_size(t, t.auto_size, t.auto_size_min, v)
        }),
        op!("rich_text", text::set_rich_text),
        op!("raycast_target", text::set_raycast_target),
        with!("outline_width", |t, v: f32| text::set_outline(t, v, t.outline_color)),
        with!("outline_color", |t, v: Vec4| text::set_outline(t, t.outline_width, v)),
        with!("shadow_offset", |t, v: Vec2| text::set_shadow(t, v, t.shadow_color)),
        with!("shadow_color", |t, v: Vec4| text::set_shadow(t, t.shadow_offset, v)),
        with!("glow_size", |t, v: f32| text::set_glow(t, v, t.glow_color)),
        with!("glow_color", |t, v: Vec4| text::set_glow(t, t.glow_size, v)),
        op!("blend", text::set_blend),
        with!("shader", |t, v: Option<UiShader>| set_ui_shader(&mut t.shader, v)?),
    ];
    ShapeComponent => shape, shape_mut, &[
        op!("kind", shape::set_kind),
        op!("corner", shape::set_corner),
        op!("radius", shape::set_radius),
        op!("inner_radius", shape::set_inner_radius),
        with!("arc_start", |s, v: f32| shape::set_arc(s, v, s.arc_end)),
        with!("arc_end", |s, v: f32| shape::set_arc(s, s.arc_start, v)),
        op!("thickness", shape::set_thickness),
        with!("dash", |s, v: f32| shape::set_dash(s, v, s.gap)),
        with!("gap", |s, v: f32| shape::set_dash(s, s.dash, v)),
        op!("color", shape::set_color),
        op!("gradient", shape::set_gradient),
        with!("border_width", |s, v: f32| shape::set_border(s, v, s.border_color)),
        with!("border_color", |s, v: Vec4| shape::set_border(s, s.border_width, v)),
        with!("shadow", |s, v: ShapeShadow| shape::set_shadow(s, v.offset, v.blur, v.color)),
        with!("glow", |s, v: ShapeGlow| shape::set_glow(s, v.size, v.intensity, v.color)),
        op!("blend", shape::set_blend),
        op!("raycast_target", shape::set_raycast_target),
        with!("shader", |s, v: Option<UiShader>| set_ui_shader(&mut s.shader, v)?),
    ];
}
