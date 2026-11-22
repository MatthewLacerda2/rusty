use glam::Vec3;

use super::cascades::{Cascade, BLEND_FRACTION, MAX_CASCADES};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CascadeUniform {
    light_space: [[f32; 16]; MAX_CASCADES],
    splits: [f32; MAX_CASCADES],
    texels: [f32; MAX_CASCADES],
    depth_ranges: [f32; MAX_CASCADES],
    view_pos: [f32; 4],
    view_dir: [f32; 4],
    params: [f32; 4],
}

impl CascadeUniform {
    pub(crate) fn new(cascades: &[Cascade], position: Vec3, forward: Vec3) -> Self {
        let mut u = Self {
            view_pos: position.extend(1.0).to_array(),
            view_dir: forward.normalize_or_zero().extend(0.0).to_array(),
            ..bytemuck::Zeroable::zeroed()
        };
        for (i, c) in cascades.iter().take(MAX_CASCADES).enumerate() {
            u.light_space[i] = c.light_space.to_cols_array();
            u.splits[i] = c.split;
            u.texels[i] = c.texel;
            u.depth_ranges[i] = c.depth_range;
        }
        let distance = cascades.last().map_or(0.0, |c| c.split);
        u.params = [cascades.len() as f32, distance, BLEND_FRACTION, 0.0];
        u
    }
}
