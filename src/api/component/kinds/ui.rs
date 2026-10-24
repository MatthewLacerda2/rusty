//! src/api/component/kinds/ui.rs — the UI layout and container components' field
//! tables: `Canvas`, `RectTransform`, `Image`, `CanvasGroup`, `RectMask`, `Mask`,
//! `BackdropFilter`, `Selectable`, `LayoutGroup` and `LayoutElement`.

use glam::Vec4;

use super::super::table::{inspect, op, with};
use super::set_ui_shader;
use crate::components::{
    BackdropFilterComponent, CanvasComponent, CanvasGroupComponent, ImageComponent, LayoutAxisFit,
    LayoutElementComponent, LayoutGroupComponent, MaskComponent, RectMaskComponent,
    RectTransformComponent, SelectableComponent, SelectionState, UiShader, WorldAnchor,
};
use crate::scene::authoring::layout_element::{self as element, SizeKind};
use crate::scene::authoring::{backdrop, canvas, canvas_group as group, image, layout_group};
use crate::scene::authoring::{rect_mask, rect_transform as rect, selectable};

inspect! {
    CanvasComponent => canvas, canvas_mut, &[
        op!("render_mode", canvas::set_render_mode),
        op!("sort_order", canvas::set_sort_order),
        op!("reference_resolution", canvas::set_reference_resolution),
        op!("match_width_or_height", canvas::set_match_width_or_height),
        op!("pixels_per_unit", canvas::set_pixels_per_unit),
        op!("plane_distance", canvas::set_plane_distance),
        op!("tilt", canvas::set_tilt),
        op!("sway", canvas::set_sway),
    ];
    RectTransformComponent => rect_transform, rect_transform_mut, &[
        op!("anchor_min", rect::set_anchor_min),
        op!("anchor_max", rect::set_anchor_max),
        op!("pivot", rect::set_pivot),
        op!("anchored_position", rect::set_anchored_position),
        op!("size_delta", rect::set_size_delta),
        with!("world_anchor", |r, v: Option<WorldAnchor>| set_world_anchor(r, v)),
    ];
    ImageComponent => image, image_mut, &[
        op!("color", image::set_color),
        op!("texture", image::set_texture),
        op!("image_type", image::set_image_type),
        op!("border", image::set_border),
        op!("fill_method", image::set_fill_method),
        op!("fill_origin", image::set_fill_origin),
        op!("fill_amount", image::set_fill_amount),
        op!("fill_clockwise", image::set_fill_clockwise),
        op!("preserve_aspect", image::set_preserve_aspect),
        op!("raycast_target", image::set_raycast_target),
        op!("gradient", image::set_gradient),
        op!("blend", image::set_blend),
        with!("shader", |i, v: Option<UiShader>| set_ui_shader(&mut i.shader, v)?),
    ];
    CanvasGroupComponent => canvas_group, canvas_group_mut, &[
        op!("alpha", group::set_alpha),
        op!("interactable", group::set_interactable),
        op!("blocks_raycasts", group::set_blocks_raycasts),
    ];
    RectMaskComponent => rect_mask, rect_mask_mut, &[
        op!("padding", rect_mask::set_padding),
        op!("feather", rect_mask::set_feather),
    ];
    MaskComponent => mask, mask_mut, &[
        op!("show_mask_graphic", rect_mask::set_show_mask_graphic),
    ];
    BackdropFilterComponent => backdrop_filter, backdrop_filter_mut, &[
        op!("blur_radius", backdrop::set_blur_radius),
        op!("tint", backdrop::set_tint),
        op!("saturation", backdrop::set_saturation),
        op!("brightness", backdrop::set_brightness),
    ];
    SelectableComponent => selectable, selectable_mut, &[
        op!("interactable", selectable::set_interactable),
        op!("transition", selectable::set_transition),
        op!("target_graphic", selectable::set_target_graphic),
        with!("colors", |s, v: [Vec4; 5]| {
            for state in SelectionState::ALL {
                selectable::set_color(s, state, v[state as usize]);
            }
        }),
        op!("fade_duration", selectable::set_fade_duration),
        // Lists of optional values are read with a short or holed list in mind:
        // Lua drops a `nil`, so a missing entry is `None`.
        with!("sprites", |s, v: Vec<Option<String>>| {
            for state in SelectionState::ALL {
                selectable::set_sprite(s, state, v.get(state as usize).cloned().flatten());
            }
        }),
        op!("navigation", selectable::set_navigation),
        with!("select_on", |s, v: Vec<Option<u32>>| {
            for dir in 0..selectable::DIRECTIONS.len() {
                selectable::set_select_on(s, dir, v.get(dir).copied().flatten());
            }
        }),
    ];
    LayoutGroupComponent => layout_group, layout_group_mut, &[
        op!("kind", layout_group::set_kind),
        op!("padding", layout_group::set_padding),
        op!("spacing", layout_group::set_spacing),
        op!("child_alignment", layout_group::set_child_alignment),
        with!("control_child_width", |g, v: bool| {
            layout_group::set_control_child_size(g, v, g.control_child_height)
        }),
        with!("control_child_height", |g, v: bool| {
            layout_group::set_control_child_size(g, g.control_child_width, v)
        }),
        with!("child_force_expand_width", |g, v: bool| {
            layout_group::set_child_force_expand(g, v, g.child_force_expand_height)
        }),
        with!("child_force_expand_height", |g, v: bool| {
            layout_group::set_child_force_expand(g, g.child_force_expand_width, v)
        }),
        op!("cell_size", layout_group::set_cell_size),
        op!("constraint", layout_group::set_constraint),
        op!("constraint_count", layout_group::set_constraint_count),
        op!("start_corner", layout_group::set_start_corner),
        op!("start_vertical", layout_group::set_start_vertical),
    ];
    LayoutElementComponent => layout_element, layout_element_mut, &[
        op!("ignore_layout", element::set_ignore_layout),
        with!("min_width", |e, v: Option<f32>| element::set_size(e, SizeKind::Min, v, e.min_height)),
        with!("min_height", |e, v: Option<f32>| element::set_size(e, SizeKind::Min, e.min_width, v)),
        with!("preferred_width", |e, v: Option<f32>| {
            element::set_size(e, SizeKind::Preferred, v, e.preferred_height)
        }),
        with!("preferred_height", |e, v: Option<f32>| {
            element::set_size(e, SizeKind::Preferred, e.preferred_width, v)
        }),
        with!("flexible_width", |e, v: Option<f32>| {
            element::set_size(e, SizeKind::Flexible, v, e.flexible_height)
        }),
        with!("flexible_height", |e, v: Option<f32>| {
            element::set_size(e, SizeKind::Flexible, e.flexible_width, v)
        }),
        with!("horizontal_fit", |e, v: LayoutAxisFit| element::set_fit(e, v, e.vertical_fit)),
        with!("vertical_fit", |e, v: LayoutAxisFit| element::set_fit(e, e.horizontal_fit, v)),
    ];
}

/// A rect's world anchor, whole: `None` detaches it.
fn set_world_anchor(r: &mut RectTransformComponent, anchor: Option<WorldAnchor>) {
    let Some(a) = anchor else {
        return rect::clear_world_anchor(r);
    };
    rect::set_world_anchor(r, a.target, a.offset);
    rect::set_anchor_clamp(r, a.clamp_to_screen_edge, a.edge_padding);
    rect::set_anchor_rotate(r, a.rotate_toward_target);
    rect::set_anchor_hide_when_behind(r, a.hide_when_behind);
}
