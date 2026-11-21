use crate::gpu::ops::Writes;
use std::rc::Rc;

use crate::draw::materials::material_group;
use crate::gpu::bind_layouts;
use crate::gpu::material_cache::MATERIAL_MAPS;
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::textures::MipGenerator;
use crate::{GpuTexture, Renderer};

pub(crate) struct Textures {
    pub texture_layout: wgpu::BindGroupLayout,
    pub material_layout: wgpu::BindGroupLayout,
    pub default_texture: Rc<GpuTexture>,
    pub white_texture: Rc<GpuTexture>,
    pub default_material_bind_group: wgpu::BindGroup,
    pub zero_params: wgpu::Buffer,
    pub mip_generator: MipGenerator,
}

pub(crate) fn create_textures(
    device: &wgpu::Device,
    writes: Writes,
    registry: &mut ShaderRegistry,
) -> Textures {
    let texture_layout = bind_layouts::create_texture_layout(device);
    let material_layout = bind_layouts::create_material_layout(device);
    let mip_generator = MipGenerator::new(device, registry);
    let gpu = (device, writes, &mip_generator);
    let default_texture = Rc::new(Renderer::create_default_checkerboard_texture(
        gpu,
        &texture_layout,
    ));
    let white_texture = Rc::new(Renderer::create_white_texture(
        (device, writes),
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

fn create_default_material_bind_group(
    device: &wgpu::Device,
    material_layout: &wgpu::BindGroupLayout,
    [default_texture, white_texture]: [&GpuTexture; 2],
    zero_params: &wgpu::Buffer,
) -> wgpu::BindGroup {
    let maps = std::array::from_fn(|i| match i < MATERIAL_MAPS {
        true => default_texture,
        false => white_texture,
    });
    material_group(device, material_layout, maps, zero_params)
}
