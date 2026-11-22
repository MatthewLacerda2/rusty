use super::headless_or_skip;
use crate::Renderer;

pub fn culling_or_skip(width: u32, height: u32) -> Option<Renderer> {
    let mut r = floored_or_skip(width, height)?;
    r.hiz.min_instances = 0;
    Some(r)
}

pub fn floored_or_skip(width: u32, height: u32) -> Option<Renderer> {
    let mut r = headless_or_skip(width, height)?;
    r.quality.gpu_occlusion_culling = true;
    r.quality.gpu_cluster_culling = true;
    Some(r)
}
