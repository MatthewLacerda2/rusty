//! A headless renderer with the GPU occlusion cull on, for the tests about it.

use super::headless_or_skip;
use crate::Renderer;

/// [`headless_or_skip`] with the GPU occlusion cull and its cluster cull on: the
/// seeded presets ship both off (#2388), so a test about the cull turns it on here.
pub fn culling_or_skip(width: u32, height: u32) -> Option<Renderer> {
    let mut r = headless_or_skip(width, height)?;
    r.quality.gpu_occlusion_culling = true;
    r.quality.gpu_cluster_culling = true;
    Some(r)
}
