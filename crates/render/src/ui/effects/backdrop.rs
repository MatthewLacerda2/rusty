use crate::ui::mesh::{close_run, push_graphic, CanvasMesh, Coverage, Frame, Inherited};
use rusty_components::UiBlend;
use rusty_ui::UiRect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiBackdrop {
    pub(crate) radius: f32,
    pub(crate) tint: [f32; 4],
    pub(crate) filter: [f32; 2],
}

pub(crate) const MAX_LEVEL: u32 = 6;

pub(crate) fn blur_level(radius: f32, divisor: u32) -> u32 {
    let reach = radius / (2.0 * divisor.max(1) as f32);
    if reach <= 1.0 {
        return 0;
    }
    (reach.log2().ceil() as u32).min(MAX_LEVEL)
}

pub(in crate::ui) fn push_backdrop(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    id: u32,
    rect: &UiRect,
    state: Inherited,
) {
    if !frame.overlay {
        return;
    }
    let Some(filter) = frame.world.backdrop_filter(id).map(|b| (*b).clone()) else {
        return;
    };
    let Some(clip) = state.visible_clip(state.alpha, frame.screen) else {
        return;
    };
    let backdrop = UiBackdrop {
        radius: filter.blur_radius * rect.scale_factor,
        tint: filter.tint.to_array(),
        filter: [filter.saturation, filter.brightness],
    };
    let start = mesh.vertices.len() as u32;
    let source = push_graphic(mesh, frame, id, rect, Coverage::Flat(state.alpha));
    close_run(
        mesh,
        (source, UiBlend::Normal, Some(backdrop), None),
        clip,
        start,
    );
}
