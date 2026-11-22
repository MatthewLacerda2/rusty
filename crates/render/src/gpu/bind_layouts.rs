use crate::draw::map_slots::all_bindings;
use crate::gpu::ops::CountedDevice as _;
use crate::passes::transparent::OpaqueViews;

use super::layout_entries::{
    depth_entry, dynamic_uniform_entry, sampler_entry, storage_entry, texture_entry, uniform_entry,
};

pub(crate) fn create_camera_lighting_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera, Lighting, Skybox & Reflection Layout"),
        entries: &camera_lighting_entries(),
    })
}

/// Group 0 of `forward/bindings.wgsl`, `particles.wgsl`, `skybox.wgsl`, `sky_gradient.wgsl`
/// and `sky/air.wgsl`: change them together; `groups::tests` holds only the first two.
pub(crate) fn camera_lighting_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use wgpu::TextureViewDimension::{Cube, CubeArray, D2Array, D3};
    vec![
        uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
        uniform_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
        texture_entry(2, Cube),
        texture_entry(4, CubeArray),
        storage_entry(7, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(8, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(9, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(10, wgpu::ShaderStages::FRAGMENT),
        texture_entry(11, D2Array),
        texture_entry(12, D2Array),
        sampler_entry(13),
        texture_entry(14, D2Array),
        texture_entry(16, D2Array),
        wgpu::BindGroupLayoutEntry {
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ..texture_entry(17, D3)
        },
        wgpu::BindGroupLayoutEntry {
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ..sampler_entry(18)
        },
        texture_entry(19, D2Array),
    ]
}

pub(crate) fn create_scene_depth_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Scene Depth Layout"),
        entries: &scene_depth_entries(),
    })
}

pub(crate) fn scene_depth_entries() -> [wgpu::BindGroupLayoutEntry; 1] {
    [depth_entry(0, wgpu::TextureViewDimension::D2)]
}

pub(crate) fn create_entity_bones_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Entity & Bones Layout"),
        entries: &entity_bones_entries(),
    })
}

pub(crate) fn entity_bones_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use crate::DrawUniform;
    vec![
        dynamic_uniform_entry(
            0,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            std::mem::size_of::<DrawUniform>() as u64,
        ),
        storage_entry(1, wgpu::ShaderStages::VERTEX),
        storage_entry(2, wgpu::ShaderStages::VERTEX),
        storage_entry(4, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(5, wgpu::ShaderStages::FRAGMENT),
        storage_entry(6, wgpu::ShaderStages::VERTEX_FRAGMENT),
    ]
}

pub(crate) fn create_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Texture Layout"),
        entries: &texture_entries(),
    })
}

pub(crate) fn texture_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

/// Bytes: a material's runtime shader-param uniform.
pub(crate) const PARAMS_SIZE: u64 =
    std::mem::size_of::<rusty_shadergen::params::PackedParams>() as u64;

fn material_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    texture_entry(binding, wgpu::TextureViewDimension::D2)
}

fn material_sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

pub(crate) fn create_material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Material Layout"),
        entries: &material_entries(),
    })
}

pub(crate) fn material_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use rusty_shadergen::textures::sampler_binding;
    let params = wgpu::BindGroupLayoutEntry {
        binding: rusty_shadergen::params::PARAM_BINDING,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(PARAMS_SIZE),
        },
        count: None,
    };
    let mut entries = vec![material_sampler_entry(1), params];
    for binding in all_bindings() {
        entries.push(material_texture_entry(binding));
        if binding != 0 {
            entries.push(material_sampler_entry(sampler_binding(binding)));
        }
    }
    entries
}

pub(crate) fn create_shadow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Main Shadow Bind Group Layout"),
        entries: &shadow_entries(),
    })
}

pub(crate) fn shadow_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use wgpu::TextureViewDimension::{D2Array, D2};
    vec![
        uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
        depth_entry(1, D2Array),
        wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            count: None,
        },
        texture_entry(3, D2),
        depth_entry(4, D2),
        storage_entry(5, wgpu::ShaderStages::FRAGMENT),
        texture_entry(6, D2),
        sampler_entry(7),
    ]
}

pub(crate) fn create_shadow_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    shadows: &crate::passes::shadows::ShadowRenderer,
    ao: &wgpu::TextureView,
    opaque: OpaqueViews,
) -> wgpu::BindGroup {
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Main Shadow Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&shadows.active_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&shadows.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(ao),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&shadows.atlas.view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: shadows.atlas.tiles.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(opaque.texture),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(opaque.sampler),
            },
        ],
    })
}
