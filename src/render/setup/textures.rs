//! The forward renderer's group(2) texture setup: the single-texture + expanded
//! material bind-group layouts, the default checker texture, and the all-default
//! material bind group. A cohesive unit ([`Textures`]) that `setup_build` builds and
//! `Renderer::from_parts` flattens into the renderer.

use std::rc::Rc;

use super::bind_layouts;
use super::{GpuTexture, Renderer};

/// The group(2) texture layouts and the default texture/material bind group.
pub(super) struct Textures {
    pub texture_layout: wgpu::BindGroupLayout,
    pub material_layout: wgpu::BindGroupLayout,
    pub default_texture: Rc<GpuTexture>,
    pub default_material_bind_group: wgpu::BindGroup,
}

/// Build the single-texture + expanded-material group(2) layouts, the default
/// checker texture, and the all-default material bind group (#202).
pub(super) fn create_textures(device: &wgpu::Device, queue: &wgpu::Queue) -> Textures {
    let texture_layout = bind_layouts::create_texture_layout(device);
    let material_layout = bind_layouts::create_material_layout(device);
    let default_texture = Rc::new(Renderer::create_default_checkerboard_texture(
        device,
        queue,
        &texture_layout,
    ));
    let default_material_bind_group =
        create_default_material_bind_group(device, &material_layout, &default_texture);
    Textures {
        texture_layout,
        material_layout,
        default_texture,
        default_material_bind_group,
    }
}

/// A group(2) material bind group with all five texture slots (albedo, metallic,
/// roughness, normal, emissive) pointing at the default texture (one shared sampler).
/// Bound by passes that need group(2) but never sample the maps (outline, editor
/// grid) (#202, #207).
fn create_default_material_bind_group(
    device: &wgpu::Device,
    material_layout: &wgpu::BindGroupLayout,
    default_texture: &GpuTexture,
) -> wgpu::BindGroup {
    let view = wgpu::BindingResource::TextureView(&default_texture.view);
    // Textures at 0,2,3,4,5; sampler at 1.
    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 1,
        resource: wgpu::BindingResource::Sampler(&default_texture.sampler),
    }];
    for binding in [0, 2, 3, 4, 5] {
        entries.push(wgpu::BindGroupEntry {
            binding,
            resource: view.clone(),
        });
    }
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Default Material Bind Group"),
        layout: material_layout,
        entries: &entries,
    })
}
