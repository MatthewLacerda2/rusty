//! src/ui/layout/fit.rs — the content size fitter (#421).
//!
//! Unity's `ContentSizeFitter`, carried by `LayoutElement.horizontal_fit` /
//! `vertical_fit`: an element its parent does not arrange is laid out by its own
//! RectTransform, then each fitted axis is resized to the element's min or
//! preferred size around its pivot. The width is fitted first, so a wrapping text
//! box measures its height at its final width. (Under a layout group the group
//! reads the fitted size as the child's own size instead; see `sizes::own_size`.)

use glam::Vec2;

use super::sizes::element_sizes;
use crate::components::{LayoutAxisFit, RectTransformComponent};
use crate::ecs::World;

/// `rect` (`(min, size)`, laid out from `rt`) resized by `id`'s content fitter.
pub(super) fn fit(
    world: &World,
    id: u32,
    rt: &RectTransformComponent,
    rect: (Vec2, Vec2),
) -> (Vec2, Vec2) {
    let Some(le) = world.layout_element(id) else {
        return rect;
    };
    let (min, mut size) = rect;
    let pivot = min + size * rt.pivot;
    for axis in 0..2 {
        let sizes = || element_sizes(world, id, axis, size.x, 0);
        size[axis] = match le.fit(axis) {
            LayoutAxisFit::Unconstrained => continue,
            LayoutAxisFit::MinSize => sizes().min,
            LayoutAxisFit::PreferredSize => sizes().preferred,
        };
    }
    (pivot - size * rt.pivot, size)
}

#[cfg(test)]
#[path = "fit_tests.rs"]
mod fit_tests;
