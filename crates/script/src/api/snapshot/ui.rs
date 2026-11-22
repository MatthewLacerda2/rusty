use serde_json::{json, Value};

use rusty_components::{
    CanvasComponent, CanvasGroupComponent, ImageComponent, RectMaskComponent,
    RectTransformComponent, ShapeComponent,
};

pub(crate) fn canvas_value(c: &CanvasComponent) -> Value {
    json!({
        "render_mode": c.render_mode.name(),
        "sort_order": c.sort_order,
        "reference_resolution": [c.reference_resolution.x, c.reference_resolution.y],
        "match_width_or_height": c.match_width_or_height,
        "pixels_per_unit": c.pixels_per_unit,
        "plane_distance": c.plane_distance,
        "tilt": [c.tilt.x, c.tilt.y],
        "sway": c.sway,
    })
}

pub(crate) fn rect_transform_value(r: &RectTransformComponent) -> Value {
    let v = |v: glam::Vec2| json!([v.x, v.y]);
    json!({
        "anchor_min": v(r.anchor_min),
        "anchor_max": v(r.anchor_max),
        "pivot": v(r.pivot),
        "anchored_position": v(r.anchored_position),
        "size_delta": v(r.size_delta),
        "world_anchor": r.world_anchor.as_ref().map(|a| json!({
            "target": a.target,
            "target_lost": a.target_lost,
            "offset": [a.offset.x, a.offset.y, a.offset.z],
            "clamp_to_screen_edge": a.clamp_to_screen_edge,
            "edge_padding": a.edge_padding,
            "rotate_toward_target": a.rotate_toward_target,
            "hide_when_behind": a.hide_when_behind,
        })),
    })
}

pub(crate) fn image_value(i: &ImageComponent) -> Value {
    let v4 = |v: glam::Vec4| json!([v.x, v.y, v.z, v.w]);
    json!({
        "color": v4(i.color),
        "texture": i.texture,
        "type": i.image_type.name(),
        "border": v4(i.border),
        "fill_method": i.fill_method.name(),
        "fill_origin": i.fill_origin.name(),
        "fill_amount": i.fill_amount,
        "fill_clockwise": i.fill_clockwise,
        "preserve_aspect": i.preserve_aspect,
        "raycast_target": i.raycast_target,
        "gradient": i.gradient,
        "blend": i.blend.name(),
    })
}

pub(crate) fn shape_value(s: &ShapeComponent) -> Value {
    let v4 = |v: glam::Vec4| json!([v.x, v.y, v.z, v.w]);
    json!({
        "kind": s.kind.name(),
        "corner": s.corner.name(),
        "radius": v4(s.radius),
        "inner_radius": s.inner_radius,
        "arc": [s.arc_start, s.arc_end],
        "thickness": s.thickness,
        "dash": [s.dash, s.gap],
        "color": v4(s.color),
        "gradient": s.gradient,
        "border": { "width": s.border_width, "color": v4(s.border_color) },
        "shadow": {
            "offset": [s.shadow.offset.x, s.shadow.offset.y],
            "blur": s.shadow.blur,
            "color": v4(s.shadow.color),
        },
        "glow": { "size": s.glow.size, "intensity": s.glow.intensity, "color": v4(s.glow.color) },
        "blend": s.blend.name(),
        "raycast_target": s.raycast_target,
    })
}

pub(crate) fn canvas_group_value(g: &CanvasGroupComponent) -> Value {
    json!({
        "alpha": g.alpha,
        "interactable": g.interactable,
        "blocks_raycasts": g.blocks_raycasts,
    })
}

pub(crate) fn selectable_value(s: &rusty_components::SelectableComponent) -> Value {
    use rusty_components::SelectionState;
    use rusty_edit::authoring::ui::selectable as ops;
    let colors: serde_json::Map<String, Value> = SelectionState::ALL
        .iter()
        .map(|&st| (st.name().to_string(), json!(s.color(st).to_array())))
        .collect();
    let sprites: serde_json::Map<String, Value> = SelectionState::ALL
        .iter()
        .map(|&st| (st.name().to_string(), json!(s.sprite(st))))
        .collect();
    let select_on: serde_json::Map<String, Value> = ops::DIRECTIONS
        .iter()
        .zip(s.select_on)
        .map(|(d, t)| (d.to_string(), json!(t)))
        .collect();
    json!({
        "interactable": s.interactable,
        "transition": s.transition.name(),
        "target_graphic": s.target_graphic,
        "colors": colors,
        "fade_duration": s.fade_duration,
        "sprites": sprites,
        "navigation": s.navigation.name(),
        "select_on": select_on,
    })
}

pub(crate) fn rect_mask_value(m: &RectMaskComponent) -> Value {
    let p = m.padding;
    json!({ "padding": [p.x, p.y, p.z, p.w], "feather": m.feather })
}

pub(crate) fn mask_value(m: &rusty_components::MaskComponent) -> Value {
    json!({ "show_mask_graphic": m.show_mask_graphic })
}

pub(crate) fn backdrop_filter_value(b: &rusty_components::BackdropFilterComponent) -> Value {
    let t = b.tint;
    json!({
        "blur_radius": b.blur_radius,
        "tint": [t.x, t.y, t.z, t.w],
        "saturation": b.saturation,
        "brightness": b.brightness,
    })
}

pub(crate) fn text_value(t: &rusty_components::TextComponent) -> Value {
    let v4 = |v: glam::Vec4| json!([v.x, v.y, v.z, v.w]);
    json!({
        "text": t.text,
        "font": t.font,
        "font_bold": t.font_bold,
        "font_italic": t.font_italic,
        "font_size": t.font_size,
        "color": v4(t.color),
        "alignment": t.alignment.name(),
        "wrap": t.wrap,
        "overflow": t.overflow.name(),
        "line_spacing": t.line_spacing,
        "letter_spacing": t.letter_spacing,
        "auto_size": [t.auto_size, t.auto_size_min, t.auto_size_max],
        "rich_text": t.rich_text,
        "raycast_target": t.raycast_target,
        "outline": { "width": t.outline_width, "color": v4(t.outline_color) },
        "shadow": { "offset": [t.shadow_offset.x, t.shadow_offset.y], "color": v4(t.shadow_color) },
        "glow": { "size": t.glow_size, "color": v4(t.glow_color) },
        "blend": t.blend.name(),
    })
}

pub(crate) fn layout_group_value(g: &rusty_components::LayoutGroupComponent) -> Value {
    let p = g.padding;
    json!({
        "kind": g.kind.name(),
        "padding": [p.x, p.y, p.z, p.w],
        "spacing": [g.spacing.x, g.spacing.y],
        "child_alignment": g.child_alignment.name(),
        "control_child_size": [g.control_child_width, g.control_child_height],
        "child_force_expand": [g.child_force_expand_width, g.child_force_expand_height],
        "cell_size": [g.cell_size.x, g.cell_size.y],
        "constraint": g.constraint.name(),
        "constraint_count": g.constraint_count,
        "start_corner": g.start_corner.name(),
        "start_vertical": g.start_vertical,
    })
}

pub(crate) fn layout_element_value(e: &rusty_components::LayoutElementComponent) -> Value {
    json!({
        "ignore_layout": e.ignore_layout,
        "min_size": [e.min_width, e.min_height],
        "preferred_size": [e.preferred_width, e.preferred_height],
        "flexible_size": [e.flexible_width, e.flexible_height],
        "fit": [e.horizontal_fit.name(), e.vertical_fit.name()],
    })
}

super::fields::snapshot! {
    CanvasComponent => canvas_value,
    RectTransformComponent => rect_transform_value,
    ImageComponent => image_value,
    ShapeComponent => shape_value,
    CanvasGroupComponent => canvas_group_value,
    rusty_components::SelectableComponent => selectable_value,
    RectMaskComponent => rect_mask_value,
    rusty_components::MaskComponent => mask_value,
    rusty_components::BackdropFilterComponent => backdrop_filter_value,
    rusty_components::TextComponent => text_value,
    rusty_components::LayoutGroupComponent => layout_group_value,
    rusty_components::LayoutElementComponent => layout_element_value,
}
