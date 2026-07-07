use crate::render::gpu::mesh::Vertex;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::{transform_aabb, Frustum};
use crate::scene::Scene;
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

pub struct ShadowRenderer {
    pub static_texture: wgpu::Texture,
    pub static_view: wgpu::TextureView,
    pub active_texture: wgpu::Texture,
    pub active_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,

    pipeline: wgpu::RenderPipeline,
    light_space_buffer: wgpu::Buffer,
    pub light_space_matrix: Mat4,

    pub is_static_cached: bool,

    global_bind_group: wgpu::BindGroup,
    entity_layout: wgpu::BindGroupLayout,

    /// Persistent per-entity model-matrix buffers + bind groups, keyed by entity id.
    /// The depth pass used to `create_buffer_init` one buffer + bind group per entity
    /// every frame (static *and* dynamic); these are written in place with
    /// `queue.write_buffer` and reused instead (#210).
    entity_slots: HashMap<u32, ShadowSlot>,
}

/// One entity's reused shadow-pass model-matrix buffer + its bind group.
struct ShadowSlot {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl ShadowRenderer {
    pub const SHADOW_SIZE: u32 = 2048;

    pub fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let (static_texture, static_view, active_texture, active_view) =
            Self::create_depth_textures(device);
        let (sampler, bind_group_layout, bind_group) =
            Self::create_sampler_and_bind_group(device, &active_view);

        let light_space_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Light Space Buffer"),
            size: 64, // Mat4 size
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (global_layout, global_bind_group, entity_layout) =
            Self::create_pass_layouts(device, &light_space_buffer);
        let pipeline = Self::create_pipeline(device, &global_layout, &entity_layout, registry);

        Self {
            static_texture,
            static_view,
            active_texture,
            active_view,
            sampler,
            bind_group_layout,
            bind_group,
            pipeline,
            light_space_buffer,
            light_space_matrix: Mat4::IDENTITY,
            is_static_cached: false,
            global_bind_group,
            entity_layout,
            entity_slots: HashMap::new(),
        }
    }

    /// Allocate the static + active depth textures (and their default views).
    fn create_depth_textures(
        device: &wgpu::Device,
    ) -> (
        wgpu::Texture,
        wgpu::TextureView,
        wgpu::Texture,
        wgpu::TextureView,
    ) {
        let size = wgpu::Extent3d {
            width: Self::SHADOW_SIZE,
            height: Self::SHADOW_SIZE,
            depth_or_array_layers: 1,
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("Shadow Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let static_texture = device.create_texture(&desc);
        let static_view = static_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let active_texture = device.create_texture(&desc);
        let active_view = active_texture.create_view(&wgpu::TextureViewDescriptor::default());

        (static_texture, static_view, active_texture, active_view)
    }

    /// Comparison sampler plus the layout/group that expose the depth map to the
    /// main shader.
    fn create_sampler_and_bind_group(
        device: &wgpu::Device,
        active_view: &wgpu::TextureView,
    ) -> (wgpu::Sampler, wgpu::BindGroupLayout, wgpu::BindGroup) {
        // Sampler with comparison for hardware PCF shadows
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        // Bind group layout to expose shadow depth map to main shader
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Map Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Map Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(active_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        (sampler, bind_group_layout, bind_group)
    }

    /// Global (light-space) and per-entity layouts used by the depth-only pass.
    fn create_pass_layouts(
        device: &wgpu::Device,
        light_space_buffer: &wgpu::Buffer,
    ) -> (
        wgpu::BindGroupLayout,
        wgpu::BindGroup,
        wgpu::BindGroupLayout,
    ) {
        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Global Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let global_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Global Bind Group"),
            layout: &global_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: light_space_buffer.as_entire_binding(),
            }],
        });

        let entity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Entity Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        (global_layout, global_bind_group, entity_layout)
    }

    /// Depth-only render pipeline (sloped bias to fight shadow acne).
    fn create_pipeline(
        device: &wgpu::Device,
        global_layout: &wgpu::BindGroupLayout,
        entity_layout: &wgpu::BindGroupLayout,
        registry: &mut ShaderRegistry,
    ) -> wgpu::RenderPipeline {
        let shader = registry.load(device, "shadow.wgsl", "Shadow Shader");

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Pipeline Layout"),
            bind_group_layouts: &[global_layout, entity_layout],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: None, // Depth only pass
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2, // Sloped depth bias to prevent shadow acne
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    /// Drop shadow slots for entities no longer in `live`, keeping the per-entity
    /// buffer pool bounded to the active scene (#210).
    pub fn retain_entities(&mut self, live: &std::collections::HashSet<u32>) {
        self.entity_slots.retain(|id, _| live.contains(id));
    }

    pub fn update_light_space(&mut self, queue: &wgpu::Queue, light_dir: Vec3) {
        let norm_dir = light_dir.normalize();
        // Position the shadow camera looking at the center of the scene
        let center = Vec3::ZERO;
        let shadow_cam_pos = center - norm_dir * 45.0;
        let view = Mat4::look_at_rh(shadow_cam_pos, center, Vec3::Y);

        // Orthographic projection suitable for typical scenes
        let proj = Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 1.0, 100.0);
        self.light_space_matrix = proj * view;

        queue.write_buffer(
            &self.light_space_buffer,
            0,
            bytemuck::bytes_of(&self.light_space_matrix.to_cols_array()),
        );
    }

    pub fn render_static(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        gpu_meshes: &HashMap<crate::render::MeshId, crate::render::GpuMesh>,
    ) {
        let render_resources =
            self.collect_entity_resources(device, queue, scene, gpu_meshes, true);

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Static Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.static_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.draw_resources(&mut render_pass, gpu_meshes, &render_resources);
        }

        self.is_static_cached = true;
    }

    pub fn render_dynamic(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        gpu_meshes: &HashMap<crate::render::MeshId, crate::render::GpuMesh>,
    ) {
        let size = wgpu::Extent3d {
            width: Self::SHADOW_SIZE,
            height: Self::SHADOW_SIZE,
            depth_or_array_layers: 1,
        };

        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.static_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &self.active_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            size,
        );

        let render_resources =
            self.collect_entity_resources(device, queue, scene, gpu_meshes, false);

        if !render_resources.is_empty() {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Dynamic Render Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.active_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.draw_resources(&mut render_pass, gpu_meshes, &render_resources);
        }
    }

    /// Sync the persistent model-matrix buffer for every active entity whose
    /// `is_static` flag matches `want_static`, returning a lightweight draw item per
    /// entity. The buffer + bind group are reused across frames and only the matrix is
    /// rewritten with `queue.write_buffer` (#210).
    fn collect_entity_resources(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        gpu_meshes: &HashMap<crate::render::MeshId, crate::render::GpuMesh>,
        want_static: bool,
    ) -> Vec<(crate::render::MeshId, u32, u32)> {
        // Cull casters against the LIGHT's frustum, not the camera's (#330). An off-screen
        // caster still inside the light's ortho volume must keep its shadow — culling
        // casters by the player camera is the classic pop-a-shadow bug.
        let frustum = Frustum::from_view_proj(self.light_space_matrix);
        let mut render_resources = Vec::new();
        for id in scene.world.ids_with_mesh() {
            if !scene.world.is_active(id) || scene.world.is_static(id) != want_static {
                continue;
            }
            let mesh = scene.world.mesh(id).expect("id came from ids_with_mesh");
            let mesh_id = crate::render::MeshId::from_mesh(&mesh);
            let Some(gpu_mesh) = gpu_meshes.get(&mesh_id) else {
                continue;
            };
            let world = scene.world_matrix(id);
            // Skinned casters are never culled — their AABB is the rest pose, which an
            // animation can exceed; a wrongly-culled caster would drop its shadow (#330).
            if !mesh.is_skinned() {
                let (amin, amax) =
                    transform_aabb(gpu_mesh.local_aabb.0, gpu_mesh.local_aabb.1, world);
                if !frustum.intersects_aabb(amin, amax) {
                    continue;
                }
            }
            self.sync_entity_slot(device, queue, id, &world.to_cols_array());
            render_resources.push((mesh_id, id, gpu_mesh.num_indices));
        }
        render_resources
    }

    /// Get-or-create entity `id`'s shadow slot, writing the model matrix into its
    /// persistent buffer (allocated once, on first appearance).
    fn sync_entity_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: u32,
        model_arr: &[f32; 16],
    ) {
        if let Some(slot) = self.entity_slots.get(&id) {
            queue.write_buffer(&slot.buffer, 0, bytemuck::bytes_of(model_arr));
            return;
        }
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Entity Uniform"),
            contents: bytemuck::bytes_of(model_arr),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Entity Bind Group"),
            layout: &self.entity_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        self.entity_slots
            .insert(id, ShadowSlot { buffer, bind_group });
    }

    /// Issue the depth-only draw calls for collected entity resources, pulling each
    /// entity's reused bind group from the slot pool.
    fn draw_resources<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        gpu_meshes: &'a HashMap<crate::render::MeshId, crate::render::GpuMesh>,
        render_resources: &'a [(crate::render::MeshId, u32, u32)],
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.global_bind_group, &[]);

        for (mesh_id, id, num_indices) in render_resources {
            let (Some(gpu_mesh), Some(slot)) = (gpu_meshes.get(mesh_id), self.entity_slots.get(id))
            else {
                continue;
            };
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, &slot.bind_group, &[]);
            render_pass.draw_indexed(0..*num_indices, 0, 0..1);
        }
    }
}
