//! src/api/snapshot/ui.rs — the UI components' serializers for the scene-read.
//!
//! The leaf `*_value` builders for the Canvas family (#417–#420): the same stable
//! authoring JSON as `components.rs`, split out by responsibility.

use serde_json::{json, Value};

use crate::components::{
    CanvasComponent, CanvasGroupComponent, ImageComponent, RectMaskComponent,
    RectTransformComponent,
};

/// Canvas authoring view (#417): render mode, sort order and the scaler. The
/// computed rect is the entity's `ui_rect`, not part of the component.
pub(crate) fn canvas_value(c: &CanvasComponent) -> Value {
    json!({
        "render_mode": crate::scene::authoring::canvas::render_mode_name(c.render_mode),
        "sort_order": c.sort_order,
        "reference_resolution": [c.reference_resolution.x, c.reference_resolution.y],
        "match_width_or_height": c.match_width_or_height,
    })
}

/// RectTransform authoring view (#417): anchors, pivot, anchored position and size
/// delta, each `[x, y]` in reference units. The computed rect is `ui_rect`.
pub(crate) fn rect_transform_value(r: &RectTransformComponent) -> Value {
    let v = |v: glam::Vec2| json!([v.x, v.y]);
    json!({
        "anchor_min": v(r.anchor_min),
        "anchor_max": v(r.anchor_max),
        "pivot": v(r.pivot),
        "anchored_position": v(r.anchored_position),
        "size_delta": v(r.size_delta),
    })
}

/// Image authoring view (#418): tint, texture, type, 9-slice border and the fill.
/// Enum values are their `Image.*` names.
pub(crate) fn image_value(i: &ImageComponent) -> Value {
    use crate::scene::authoring::image as ops;
    let v4 = |v: glam::Vec4| json!([v.x, v.y, v.z, v.w]);
    json!({
        "color": v4(i.color),
        "texture": i.texture,
        "type": ops::image_type_name(i.image_type),
        "border": v4(i.border),
        "fill_method": ops::fill_method_name(i.fill_method),
        "fill_origin": ops::fill_origin_name(i.fill_origin),
        "fill_amount": i.fill_amount,
        "fill_clockwise": i.fill_clockwise,
        "preserve_aspect": i.preserve_aspect,
        "raycast_target": i.raycast_target,
    })
}

/// CanvasGroup authoring view (#418): subtree alpha and the interaction flags.
pub(crate) fn canvas_group_value(g: &CanvasGroupComponent) -> Value {
    json!({
        "alpha": g.alpha,
        "interactable": g.interactable,
        "blocks_raycasts": g.blocks_raycasts,
    })
}

/// Selectable authoring view (#420): its runtime state is `UI.List` /
/// `Selectable.GetState`, not part of the component.
pub(crate) fn selectable_value(s: &crate::components::SelectableComponent) -> Value {
    use crate::components::SelectionState;
    use crate::scene::authoring::selectable as ops;
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
        "transition": ops::transition_name(s.transition),
        "target_graphic": s.target_graphic,
        "colors": colors,
        "fade_duration": s.fade_duration,
        "sprites": sprites,
        "navigation": ops::navigation_name(s.navigation),
        "select_on": select_on,
    })
}

/// RectMask authoring view (#418): the clip's padding (left, bottom, right, top).
pub(crate) fn rect_mask_value(m: &RectMaskComponent) -> Value {
    let p = m.padding;
    json!({ "padding": [p.x, p.y, p.z, p.w] })
}

/// Text authoring view (#419): the string, fonts, sizing, wrapping and effects.
pub(crate) fn text_value(t: &crate::components::TextComponent) -> Value {
    use crate::scene::authoring::text as ops;
    let v4 = |v: glam::Vec4| json!([v.x, v.y, v.z, v.w]);
    json!({
        "text": t.text,
        "font": t.font,
        "font_bold": t.font_bold,
        "font_italic": t.font_italic,
        "font_size": t.font_size,
        "color": v4(t.color),
        "alignment": ops::alignment_name(t.alignment),
        "wrap": t.wrap,
        "overflow": ops::overflow_name(t.overflow),
        "line_spacing": t.line_spacing,
        "letter_spacing": t.letter_spacing,
        "auto_size": [t.auto_size, t.auto_size_min, t.auto_size_max],
        "rich_text": t.rich_text,
        "raycast_target": t.raycast_target,
        "outline": { "width": t.outline_width, "color": v4(t.outline_color) },
        "shadow": { "offset": [t.shadow_offset.x, t.shadow_offset.y], "color": v4(t.shadow_color) },
        "glow": { "size": t.glow_size, "color": v4(t.glow_color) },
    })
}

/// LayoutGroup authoring view (#421): the arrangement and its knobs. Where the
/// children land is each child's `ui_rect`.
pub(crate) fn layout_group_value(g: &crate::components::LayoutGroupComponent) -> Value {
    use crate::scene::authoring::layout_group::{name_of, CONSTRAINTS, CORNERS, KINDS};
    let p = g.padding;
    json!({
        "kind": name_of(&KINDS, g.kind),
        "padding": [p.x, p.y, p.z, p.w],
        "spacing": [g.spacing.x, g.spacing.y],
        "child_alignment": crate::scene::authoring::text::alignment_name(g.child_alignment),
        "control_child_size": [g.control_child_width, g.control_child_height],
        "child_force_expand": [g.child_force_expand_width, g.child_force_expand_height],
        "cell_size": [g.cell_size.x, g.cell_size.y],
        "constraint": name_of(&CONSTRAINTS, g.constraint),
        "constraint_count": g.constraint_count,
        "start_corner": name_of(&CORNERS, g.start_corner),
        "start_vertical": g.start_vertical,
    })
}

/// LayoutElement authoring view (#421): the size overrides as `[width, height]`
/// (`null` = the content's size) and the content fitter per axis.
pub(crate) fn layout_element_value(e: &crate::components::LayoutElementComponent) -> Value {
    use crate::scene::authoring::layout_element::FITS;
    use crate::scene::authoring::layout_group::name_of;
    json!({
        "ignore_layout": e.ignore_layout,
        "min_size": [e.min_width, e.min_height],
        "preferred_size": [e.preferred_width, e.preferred_height],
        "flexible_size": [e.flexible_width, e.flexible_height],
        "fit": [name_of(&FITS, e.horizontal_fit), name_of(&FITS, e.vertical_fit)],
    })
}
