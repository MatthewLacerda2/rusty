//! The forward pass's group 3 at setup: the shadow renderer, its cascade uniform,
//! the no-AO bind group every pass without SSAO binds, and the opaque copy's pass
//! and stand-in (#1523), which that group binds too.

use crate::render::gpu::bind_layouts;
use crate::render::passes::shadows;
use crate::render::passes::transparent::OpaqueCopyPass;
use crate::render::setup::textures::Textures;

/// Shadow renderer, its cascade uniform buffer, and the main-pass bind group that
/// samples the active cascade array.
pub(crate) struct ShadowSystem {
    pub layout: wgpu::BindGroupLayout,
    pub renderer: shadows::ShadowRenderer,
    pub uniform_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    /// The opaque copy's pass, and its 1x1 stand-in every group 3 binds (#1523).
    pub opaque_copy: OpaqueCopyPass,
}

/// Shadow renderer (over the forward shader and the atlas's blits, `shaders`), its
/// cascade uniform buffer, and the main-pass bind group that samples the active
/// cascade array — with `no_ao`, the white stand-in for SSAO, and the opaque copy's
/// stand-in (#1523), whose pass the system then holds.
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
