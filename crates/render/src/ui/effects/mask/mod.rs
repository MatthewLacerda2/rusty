pub(crate) mod gpu;
mod key;

use std::ops::Range;

use crate::ui::mesh::{push_graphic, CanvasMesh, Coverage, Frame, Inherited};
use crate::ui::mesh::{UiClip, UiSource};
use rusty_ui::UiRect;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MaskDraw {
    pub(crate) id: u32,
    pub(crate) source: UiSource,
    pub(crate) clip: UiClip,
    pub(crate) range: Range<u32>,
}

pub(in crate::ui) fn push_mask(
    mesh: &mut CanvasMesh,
    frame: &Frame,
    id: u32,
    rect: &UiRect,
    state: Inherited,
) {
    if !frame.world.has_mask(id) || !state.visible {
        return;
    }
    let Some(clip) = state.clip_on(frame.screen) else {
        return;
    };
    let start = mesh.vertices.len() as u32;
    let source = push_graphic(mesh, frame, id, rect, Coverage::Own);
    let end = mesh.vertices.len() as u32;
    mesh.masks.push(MaskDraw {
        id,
        source,
        clip,
        range: start..end,
    });
}
