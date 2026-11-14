//! The forward shader's view of the cascades (#435): `ShadowCascades` in
//! `shader.wgsl`, group 3 binding 0. Everything it needs to pick a cascade for a
//! fragment and bias against it, packed into `vec4`s for WGSL's uniform layout.

use glam::Vec3;

use super::cascades::{Cascade, BLEND_FRACTION, MAX_CASCADES};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct CascadeUniform {
    light_space: [[f32; 16]; MAX_CASCADES],
    /// Per cascade: the view depth where it ends.
    splits: [f32; MAX_CASCADES],
    /// Per cascade: the world size of one texel (normal-offset and depth bias).
    texels: [f32; MAX_CASCADES],
    /// Per cascade: the world depth its `[0, 1]` spans (depth bias in map units).
    depth_ranges: [f32; MAX_CASCADES],
    /// `xyz` camera position.
    view_pos: [f32; 4],
    /// `xyz` camera forward.
    view_dir: [f32; 4],
    /// `x` cascade count, `y` shadow distance, `z` blend fraction.
    params: [f32; 4],
}

impl CascadeUniform {
    /// Pack `cascades`, fitted for a camera at `position` looking along `forward`.
    /// The shadow distance is the last cascade's split.
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
