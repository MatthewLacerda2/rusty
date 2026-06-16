//! src/render/decals.rs — URP-style box-projector decals: data + GPU resources.
//!
//! A decal is a *volume* (an oriented box), not a flat sticker. The decal pass
//! draws each box; per covered fragment it reconstructs the surface world-position
//! from the scene depth buffer, transforms into box-local space, rejects anything
//! outside the unit cube, and projects the texture along the box axes — so it
//! *wraps* whatever geometry the box overlaps, deferred-style but done forward by
//! sampling the existing depth target. This module owns the decal data + GPU
//! pipeline/buffers; per-frame draw orchestration lives in `decals_draw`. Decals
//! are render-side state (not an `Entity` component), so they stay off the
//! `--components` gate while still spawnable from a raycast hit.

use glam::{Mat4, Quat, Vec3};

use super::postfx::HDR_FORMAT;

/// Maximum decals drawn per frame. Bullet holes/scorch accumulate, but old ones
/// are evicted (FIFO) so the pass stays bounded; matches a typical FPS budget.
pub const MAX_DECALS: usize = 256;

/// One box-projector decal: an oriented volume + a texture projected through it.
/// Spawned from a surface hit (point + normal); the box is oriented so its local
/// −Z (the projection axis) points *into* the surface along the hit normal.
#[derive(Clone, Debug)]
pub struct Decal {
    /// World-space centre of the projector box.
    pub position: Vec3,
    /// Box orientation. Local −Z is the projection direction (into the surface).
    pub rotation: Quat,
    /// Full box extents (width, height, depth) in world units. Width/height size
    /// the stamp on the surface; depth is how far the projector reaches.
    pub size: Vec3,
    /// RGBA tint multiplied into the sampled texel (alpha scales the blend).
    pub color: [f32; 4],
    /// Decal texture path, or `None` for the default checker.
    pub texture: Option<String>,
}

impl Decal {
    /// Build a decal at a surface hit. `point` is the world hit position, `normal`
    /// the (outward) surface normal. `size` is width/height of the stamp; `depth`
    /// how far the box projects through the surface. The box straddles the surface
    /// and is oriented so its local −Z aims into the surface (along −normal).
    #[allow(clippy::too_many_arguments)]
    pub fn from_hit(
        point: Vec3,
        normal: Vec3,
        size: f32,
        depth: f32,
        rotation_deg: f32,
        color: [f32; 4],
        texture: Option<String>,
    ) -> Self {
        let n = normal.normalize_or_zero();
        let n = if n.length_squared() < 1.0e-6 {
            Vec3::Y
        } else {
            n
        };
        // Orient so local −Z (the projection axis) runs along −normal, into the
        // surface. `quat_look_along` builds a basis whose −Z points along `dir`.
        let mut rot = quat_look_along(-n);
        // Spin the stamp around the projection axis for variety (bullet holes).
        rot *= Quat::from_axis_angle(Vec3::Z, rotation_deg.to_radians());
        // Centre the box on the surface so the projector straddles it; the depth
        // gives margin on both sides to catch the wrapped geometry.
        let centre = point + n * (depth * 0.5);
        Self {
            position: centre,
            rotation: rot,
            size: Vec3::new(size, size, depth),
            color,
            texture,
        }
    }

    /// Object→world matrix mapping the unit cube `[-0.5, 0.5]³` to this box.
    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.size, self.rotation, self.position)
    }
}

/// Rotation whose local −Z axis points along `dir` (a "look in direction" basis),
/// with a stable up-vector fallback when `dir` is near-vertical.
fn quat_look_along(dir: Vec3) -> Quat {
    let f = dir.normalize_or_zero();
    let up = if f.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    // look_to_rh builds a view rotation; its inverse orients an object's −Z to dir.
    Quat::from_mat4(&Mat4::look_to_rh(Vec3::ZERO, f, up)).inverse()
}

/// Per-decal data uploaded to the GPU (one dynamic-uniform slot per decal).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DecalUniform {
    /// Object→world (unit cube → box).
    pub(super) model: [f32; 16],
    /// World→object (box → unit cube); used to test/project the surface point.
    pub(super) inv_model: [f32; 16],
    /// RGBA tint.
    pub(super) color: [f32; 4],
}

/// Camera globals for the decal pass: view-proj (box → clip) and the inverse
/// view-proj (depth → world surface position), plus camera position for fades.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct DecalGlobals {
    pub(super) view_proj: [f32; 16],
    pub(super) inv_view_proj: [f32; 16],
    pub(super) camera_pos: [f32; 4],
}

/// Owns every GPU resource for the decal pass. One per `Renderer`.
pub struct DecalRenderer {
    pub(super) pipeline: wgpu::RenderPipeline,
    pub(super) globals_buffer: wgpu::Buffer,
    pub(super) globals_bind_group: wgpu::BindGroup,
    pub(super) decal_layout: wgpu::BindGroupLayout,
    pub(super) depth_layout: wgpu::BindGroupLayout,
    pub(super) index_buffer: wgpu::Buffer,
    pub(super) vertex_buffer: wgpu::Buffer,
}

impl DecalRenderer {
    /// Build the decal pass: a unit-cube mesh, the globals + per-decal + depth
    /// bind-group layouts, and the projector pipeline. Reuses the renderer's
    /// `texture_layout` (group 3) for the decal sprite.
    pub fn new(device: &wgpu::Device, texture_layout: &wgpu::BindGroupLayout) -> Self {
        use wgpu::util::DeviceExt;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Decal Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/decals.wgsl").into(),
            ),
        });

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

        // Unit cube spanning [-0.5, 0.5]³ (8 corners, 36 indices).
        let (verts, indices) = super::decals_draw::unit_cube();
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
                buffers: &[super::decals_draw::decal_vertex_layout()],
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
