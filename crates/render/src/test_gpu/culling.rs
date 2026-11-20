//! A headless renderer with the GPU occlusion cull on, for the tests about it.

use super::headless_or_skip;
use crate::Renderer;

/// [`headless_or_skip`] with the GPU occlusion cull and its cluster cull on: the
/// seeded presets ship both off (#2388), so a test about the cull turns it on here.
/// It also lowers the cull's instance floor to 0 (`passes::hiz::MIN_INSTANCES`), so
/// a primitive scene of a few boxes still culls; [`floored_or_skip`] keeps it.
pub fn culling_or_skip(width: u32, height: u32) -> Option<Renderer> {
    let mut r = floored_or_skip(width, height)?;
    r.hiz.min_instances = 0;
    Some(r)
}

/// [`culling_or_skip`] with the shipped instance floor: for a test about the floor.
pub fn floored_or_skip(width: u32, height: u32) -> Option<Renderer> {
    let mut r = headless_or_skip(width, height)?;
    r.quality.gpu_occlusion_culling = true;
    r.quality.gpu_cluster_culling = true;
    Some(r)
}
