//! The limits every device is created with: wgpu's defaults, but for the sampled
//! textures one shader stage may bind (#1703).
//!
//! The forward pass's fragment stage binds 18: the 16 of wgpu's default (the sky
//! and probe cubes, the decal atlas twice, the lightmap pages twice, the material
//! maps, the cascades, the AO, the shadow atlas and the opaque copy, #1750), the
//! integrated volumetric fog (#1703) and the shadowmask pages (#1695). Every adapter rusty ships on has room: Metal
//! reports at least 31 per stage on every Apple GPU, desktop Vulkan drivers and
//! lavapipe far more. Asking for exactly what is bound costs nothing (it is a
//! validation limit, not an allocation) and fails the device loudly on an adapter
//! that cannot draw the forward pass, rather than the pipeline later.

/// The features a device takes where its adapter has them: BC texture
/// compression (#1574) and GPU occlusion culling's indirect draws (#1963). Each
/// has a path without it.
pub(crate) const OPTIONAL: wgpu::Features =
    crate::render::gpu::textures::compressed::FEATURE.union(crate::render::passes::hiz::FEATURE);

/// The most sampled textures one stage binds: the forward fragment stage's.
pub(crate) const SAMPLED_TEXTURES_PER_STAGE: u32 = 18;

/// What [`super::Renderer`]'s device asks for.
pub(crate) fn device_limits() -> wgpu::Limits {
    let defaults = wgpu::Limits::default();
    wgpu::Limits {
        max_sampled_textures_per_shader_stage: defaults
            .max_sampled_textures_per_shader_stage
            .max(SAMPLED_TEXTURES_PER_STAGE),
        ..defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forward_stages_textures_fit_and_nothing_else_moves() {
        let (limits, defaults) = (device_limits(), wgpu::Limits::default());
        assert_eq!(limits.max_sampled_textures_per_shader_stage, 18);
        let raised = wgpu::Limits {
            max_sampled_textures_per_shader_stage: 18,
            ..defaults
        };
        assert_eq!(limits, raised);
    }
}
