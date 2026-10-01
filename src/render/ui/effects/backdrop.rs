//! src/render/ui/effects/backdrop.rs — a `BackdropFilter`'s batch (#426). Pure.
//!
//! On an overlay canvas, a graphic with a BackdropFilter gets a batch of its own
//! right before it: the graphic's shape (Image texture alpha, else its Shape's
//! coverage, else its rect) with the group alpha as its only tint, so the shader reads *coverage* from it and shows
//! the blurred frame there (`ui.wgsl`'s `fs_backdrop`). The graphic then draws over
//! it as usual. The radius rides in pixels; the renderer quantizes it to a blur
//! level ([`blur_level`]), one blur per distinct level per frame.

use crate::components::UiBlend;
use crate::render::ui::mesh::{close_run, push_graphic, CanvasMesh, Coverage, Frame, Inherited};
use crate::ui::UiRect;

/// What a backdrop batch shows behind its graphic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiBackdrop {
    /// Blur radius in target pixels.
    pub(crate) radius: f32,
    /// Colour mixed over the blur by its alpha.
    pub(crate) tint: [f32; 4],
    /// Saturation and brightness.
    pub(crate) filter: [f32; 2],
}

/// The deepest blur level: the radius doubles per level, so 6 levels cover a
/// radius of several hundred pixels at any downsample.
pub(crate) const MAX_LEVEL: u32 = 6;

/// The dual-filter level blurring by about `radius` pixels when the chain starts at
/// 1/`divisor` resolution: each level doubles the reach (`2 · divisor · 2^level`
/// pixels), `0` is the downsampled frame unblurred.
pub(crate) fn blur_level(radius: f32, divisor: u32) -> u32 {
    let reach = radius / (2.0 * divisor.max(1) as f32);
    if reach <= 1.0 {
        return 0;
    }
    (reach.log2().ceil() as u32).min(MAX_LEVEL)
}

/// Emit `id`'s backdrop batch (if it has a BackdropFilter, is on an overlay canvas,
/// visible and not clipped away) into `mesh`.
pub(in crate::render::ui) fn push_backdrop(
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
