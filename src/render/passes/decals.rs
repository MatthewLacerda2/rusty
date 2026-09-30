//! src/render/passes/decals.rs — URP-style box-projector decals: GPU resources.
//!
//! A decal is a *volume* (an oriented box), not a flat sticker. The decal pass
//! draws each box; per covered fragment it reconstructs the surface world-position
//! from the scene depth buffer, transforms into box-local space, rejects anything
//! outside the unit cube, and projects the texture along the box axes — so it
//! *wraps* whatever geometry the box overlaps, deferred-style but done forward by
//! sampling the existing depth target. The decal record itself ([`Decal`]) and its
//! cap live sim-side in [`crate::scene::decal`]; this module owns the GPU
//! pipeline/buffers, and per-frame draw orchestration lives in `decals_draw`.
//!
//! [`Decal`]: crate::scene::decal::Decal

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::HDR_FORMAT;

/// Per-decal data uploaded to the GPU (one dynamic-uniform slot per decal).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DecalUniform {
    /// Object→world (unit cube → box).
    pub(crate) model: [f32; 16],
    /// World→object (box → unit cube); used to test/project the surface point.
    pub(crate) inv_model: [f32; 16],
    /// RGBA tint.
    pub(crate) color: [f32; 4],
}

/// Camera globals for the decal pass: view-proj (box → clip) and the inverse
/// view-proj (depth → world surface position), plus camera position for fades.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DecalGlobals {
    pub(crate) view_proj: [f32; 16],
    pub(crate) inv_view_proj: [f32; 16],
    pub(crate) camera_pos: [f32; 4],
}

/// Owns every GPU resource for the decal pass. One per `Renderer`.
pub struct DecalRenderer {
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) globals_buffer: wgpu::Buffer,
    pub(crate) globals_bind_group: wgpu::BindGroup,
    pub(crate) decal_layout: wgpu::BindGroupLayout,
    pub(crate) depth_layout: wgpu::BindGroupLayout,
    pub(crate) index_buffer: wgpu::Buffer,
    pub(crate) vertex_buffer: wgpu::Buffer,
}

impl DecalRenderer {
    /// Build the decal pass: a unit-cube mesh, the globals + per-decal + depth
    /// bind-group layouts, and the projector pipeline. Reuses the renderer's
    /// `texture_layout` (group 3) for the decal sprite.
    pub fn new(
        device: &wgpu::Device,
        texture_layout: &wgpu::BindGroupLayout,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let shader = registry.load(device, "decals.wgsl", "Decal Shader");

        let globals_layout = Self::globals_layout(device);
        let decal_layout = Self::decal_layout(device);
        let depth_layout = Self::depth_layout(device);

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Decal Globals"),
            size: std::mem::size_of::<DecalGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Decal Globals Bind Group"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Decal Pipeline Layout"),
            bind_group_layouts: &[
                &globals_layout,
                &decal_layout,
                &depth_layout,
                texture_layout,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = Self::pipeline(device, &shader, &pipeline_layout);
        let (vertex_buffer, index_buffer) = Self::cube_buffers(device);

        Self {
            pipeline,
            globals_buffer,
            globals_bind_group,
            decal_layout,
            depth_layout,
            index_buffer,
            vertex_buffer,
        }
    }

    /// Create the unit-cube vertex + index buffers (spanning [-0.5, 0.5]³, 8 corners,
    /// 36 indices) the projector pass draws.
    fn cube_buffers(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer) {
        use wgpu::util::DeviceExt;
        let (verts, indices) = crate::render::passes::decals_draw::unit_cube();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Decal Cube Vertices"),
            contents: bytemuck::cast_slice(&verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Decal Cube Indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        (vertex_buffer, index_buffer)
    }

    fn globals_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Decal Globals Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn decal_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Decal Per-Instance Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    fn depth_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Decal Depth Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        })
    }

    fn pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Decal Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[crate::render::passes::decals_draw::decal_vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            // Render the box's BACK faces: guarantees a covered fragment even when
            // the camera sits inside the projector volume (standard for decals).
            // No depth attachment — the pass reads scene depth as a texture and
            // rejects fragments in the shader, so it can't bind depth twice.
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }
}
