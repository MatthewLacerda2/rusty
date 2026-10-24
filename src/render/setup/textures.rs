//! The forward renderer's group(2) texture setup: the single-texture + expanded
//! material bind-group layouts, the default checker texture, and the all-default
//! material bind group. A cohesive unit ([`Textures`]) that `setup_build` builds and
//! `Renderer::from_parts` flattens into the renderer.

use std::rc::Rc;

use crate::render::draw::materials::material_group;
use crate::render::gpu::bind_layouts;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::gpu::textures::MipGenerator;
use crate::render::{GpuTexture, Renderer};

/// The group(2) texture layouts and the default texture/material bind group.
pub(crate) struct Textures {
    pub texture_layout: wgpu::BindGroupLayout,
    pub material_layout: wgpu::BindGroupLayout,
    pub default_texture: Rc<GpuTexture>,
    /// The 1×1 white an extra shader texture slot samples when a material names
    /// none, or its file is missing (#400): a neutral mask.
    pub white_texture: Rc<GpuTexture>,
    pub default_material_bind_group: wgpu::BindGroup,
    /// All-zero runtime shader params (#399), bound by every material whose shader
    /// has none — the standard shader never reads them.
    pub zero_params: wgpu::Buffer,
    /// Draws every uploaded image's mip chain (#1322).
    pub mip_generator: MipGenerator,
}

/// Build the single-texture + expanded-material group(2) layouts, the default
/// checker texture, and the all-default material bind group (#202).
pub(crate) fn create_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    registry: &mut ShaderRegistry,
) -> Textures {
    let texture_layout = bind_layouts::create_texture_layout(device);
    let material_layout = bind_layouts::create_material_layout(device);
    let mip_generator = MipGenerator::new(device, registry);
    let gpu = (device, queue, &mip_generator);
    let default_texture = Rc::new(Renderer::create_default_checkerboard_texture(
        gpu,
        &texture_layout,
    ));
    let white_texture = Rc::new(Renderer::create_white_texture(
        device,
        queue,
        &texture_layout,
    ));
    let zero_params = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Zero Shader Params"),
        size: bind_layouts::PARAMS_SIZE,
        usage: wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: false,
    });
    let default_material_bind_group = create_default_material_bind_group(
        device,
        &material_layout,
        [&default_texture, &white_texture],
        &zero_params,
    );
    Textures {
        mip_generator,
        texture_layout,
        material_layout,
        default_texture,
        white_texture,
        default_material_bind_group,
        zero_params,
    }
}

/// A group(2) material bind group with all five texture slots (albedo, metallic,
/// roughness, normal, emissive) pointing at the default texture and every extra
/// shader texture slot at white (#400).
/// Bound by passes that need group(2) but never sample the maps (the editor grid
/// and gizmos) (#202, #207).
fn create_default_material_bind_group(
    device: &wgpu::Device,
    material_layout: &wgpu::BindGroupLayout,
    [default_texture, white_texture]: [&GpuTexture; 2],
    zero_params: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let maps = std::array::from_fn(|i| match i < 5 {
        true => default_texture,
        false => white_texture,
    });
    material_group(device, material_layout, maps, zero_params)
}
