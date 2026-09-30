//! The directional-light shadow pass: a static depth map baked once per scene, copied
//! into the active map each frame, with the dynamic casters drawn over it. Casters are
//! drawn instanced, one draw per mesh (#470, `casters`).

mod casters;
mod setup;

use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::{GpuMesh, MeshId};
use crate::scene::{Scene, SceneId};
use casters::{CasterBuffer, CasterFrame};
use glam::{Mat4, Vec3};
use std::collections::HashMap;

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

    /// Which scene's static casters are currently baked into the static depth map,
    /// or `None` when it holds nothing usable (#355).
    ///
    /// This was a bare `bool`, which is the single-scene assumption in its purest
    /// form: scene A baked its statics and set the flag, then scene B saw "cached",
    /// skipped its own bake, and sampled **A's** shadows — the phantom shadows the
    /// Inspector preview showed. Naming the scene makes the cache answer the question
    /// actually being asked: not "is something baked?" but "is *this* scene baked?"
    static_cache_scene: Option<SceneId>,

    global_bind_group: wgpu::BindGroup,
    entity_layout: wgpu::BindGroupLayout,

    /// The static bake's and the dynamic pass's caster matrices (#470) — two, because
    /// both sweeps are recorded before one submit.
    static_casters: CasterBuffer,
    dynamic_casters: CasterBuffer,
    /// `(num_indices, instances)` of the caster draws since the renderer last cleared
    /// it — one frame's worth, read into the frame counters (#433).
    pub(crate) drawn: Vec<(u32, u32)>,
    /// Mirrors `Renderer::instancing`: off draws one caster per call (#470 tests).
    pub(crate) instancing: bool,
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
        let static_casters = CasterBuffer::new(device, &entity_layout);
        let dynamic_casters = CasterBuffer::new(device, &entity_layout);

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
            static_cache_scene: None,
            global_bind_group,
            entity_layout,
            static_casters,
            dynamic_casters,
            drawn: Vec::new(),
            instancing: true,
        }
    }

    /// Whether the static depth map must be re-baked for `scene`: it holds another
    /// scene's statics, or nothing at all.
    pub fn needs_static_bake(&self, scene: SceneId) -> bool {
        self.static_cache_scene != Some(scene)
    }

    /// Drop the static bake, so the next render re-bakes it. Called when the editor
    /// changes something that moves static geometry.
    pub fn invalidate_static_cache(&mut self) {
        self.static_cache_scene = None;
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
        gpu_meshes: &HashMap<MeshId, GpuMesh>,
    ) {
        let frame = CasterFrame {
            device,
            queue,
            scene,
            gpu_meshes,
        };
        let batches = self.prepare_casters(&frame, true);

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

            self.draw_casters(&mut render_pass, gpu_meshes, &batches, true);
        }

        self.static_cache_scene = Some(scene.id());
    }

    pub fn render_dynamic(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        gpu_meshes: &HashMap<MeshId, GpuMesh>,
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

        let frame = CasterFrame {
            device,
            queue,
            scene,
            gpu_meshes,
        };
        let batches = self.prepare_casters(&frame, false);

        if !batches.is_empty() {
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

            self.draw_casters(&mut render_pass, gpu_meshes, &batches, false);
        }
    }
}
