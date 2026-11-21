use crate::gpu::bind_layouts;
use crate::passes::shadows;
use crate::passes::transparent::OpaqueCopyPass;
use crate::setup::textures::Textures;

pub(crate) struct ShadowSystem {
    pub layout: wgpu::BindGroupLayout,
    pub renderer: shadows::ShadowRenderer,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub opaque_copy: OpaqueCopyPass,
}

pub(super) fn create_shadow_system(
    device: &wgpu::Device,
    shaders: [&wgpu::ShaderModule; 2],
    (no_ao, opaque_copy): (&wgpu::TextureView, OpaqueCopyPass),
    textures: &Textures,
) -> ShadowSystem {
    let layout = bind_layouts::create_shadow_layout(device);
    let renderer = shadows::ShadowRenderer::new(device, shaders, &textures.material_layout);

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Shadow Uniform Buffer"),
        size: std::mem::size_of::<shadows::CascadeUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = bind_layouts::create_shadow_bind_group(
        device,
        &layout,
        &uniform_buffer,
        &renderer,
        no_ao,
        opaque_copy.fallback(),
    );

    ShadowSystem {
        layout,
        renderer,
        uniform_buffer,
        bind_group,
        opaque_copy,
    }
}
