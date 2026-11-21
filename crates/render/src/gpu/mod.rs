//! GPU resource construction for the forward renderer: bind-group layouts,
//! pipelines, shader registry, uniform layouts, textures, mesh vertex layout,
//! and the per-frame instancing buffers + material cache (#470). Grouped here so the render root
//! lists subsystems, not individual resource files.

/// The format of every view depth target and of each pipeline that draws into one
/// (forward, prepass, skybox, particles, ribbons, world UI). Shadow maps keep their
/// own format in `passes::shadows`.
pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// One bind-group entry: `resource` at `binding`. The passes that build their groups
/// entry by entry (depth of field, motion blur) share it.
pub(crate) fn bind_entry(
    binding: u32,
    resource: wgpu::BindingResource<'_>,
) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

/// The index of the one bind group a pass outside the solid path binds, when it
/// binds only one (a blit, a compute step, a fullscreen pass).
pub(crate) const ONLY_GROUP: u32 = 0;

/// Group 0's one sampler for the engine-owned textures (binding 18): the sky's
/// reflection cube, the reflection probes' cube array, the lightmap pages with their
/// direction and shadowmask pages, and the volumetric fog volume. Linear min, mag
/// and mip filters, clamp-to-edge on all three axes, wgpu's default LOD range, no
/// anisotropy and no comparison: what each of them used to make for itself (the
/// fog's nearest mip filter aside, which its one-level volume read at LOD 0 never
/// meets). One sampler for all keeps the forward fragment stage under Metal's 16
/// (`setup::limits`).
pub(crate) fn linear_clamp(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Linear Clamp Sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

pub(crate) mod bind_layouts;
pub(crate) mod commands;
pub(crate) mod draw_buffers;
pub(crate) mod fullscreen;
pub(crate) mod global_group;
pub(crate) mod groups;
pub(crate) mod grow_buffer;
pub(crate) mod kept;
pub(crate) mod layout_entries;
pub(crate) mod lightmaps;
pub(crate) mod material_cache;
pub mod mesh;
pub(crate) mod ops;
pub(crate) mod pipelines;
pub(crate) mod shaders;
pub(crate) mod textures;
pub(crate) mod uniforms;

#[cfg(test)]
#[path = "mesh_id_tests.rs"]
mod mesh_id_tests;

#[cfg(test)]
pub(crate) mod uniform_layout_tests;
