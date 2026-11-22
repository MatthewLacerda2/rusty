pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub(crate) fn bind_entry(
    binding: u32,
    resource: wgpu::BindingResource<'_>,
) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry { binding, resource }
}

pub(crate) const ONLY_GROUP: u32 = 0;

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
