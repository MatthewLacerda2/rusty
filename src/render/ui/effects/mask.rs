//! src/render/ui/effects/mask.rs — a `Mask`'s graphic, recorded for its coverage
//! texture (#428). Pure.
//!
//! The mesh builder records each visible Mask's graphic shape as a [`MaskDraw`]:
//! the triangles of its Image, else its Shape (#425), else its rect, in the
//! graphic's own colour — its alpha is the coverage Unity's Mask writes, before any
//! CanvasGroup fade (a faded group fades the children themselves). The vertices live in the canvas's vertex
//! list like any graphic's; `mask_gpu` draws them into the mask's texture under the
//! mask's own clip, which carries every mask above it.

use std::ops::Range;

use crate::render::ui::mesh::{push_graphic, CanvasMesh, Coverage, Frame, Inherited};
use crate::render::ui::mesh::{UiClip, UiSource};
use crate::ui::UiRect;

/// One Mask's graphic: what it samples, the clip it draws under (its parents'),
/// and its vertex range.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MaskDraw {
    /// The Mask entity — what its children's [`UiClip::mask`] names.
    pub(crate) id: u32,
    pub(crate) source: UiSource,
    pub(crate) clip: UiClip,
    pub(crate) range: Range<u32>,
}

/// Record `id`'s Mask graphic (if it has a Mask and is visible) into `mesh`. A
/// mask clipped away entirely records nothing: its children are clipped away too.
pub(in crate::render::ui) fn push_mask(
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
