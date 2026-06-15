//! src/render/shadows_render.rs — per-frame shadow-map bake passes.
//!
//! The static cache pass (re-baked only when the static set changes) and the
//! dynamic pass (copies the cached static depth, then draws moving casters on
//! top). Split out of `shadows.rs` to keep both files under the size cap; the
//! `ShadowRenderer` struct, its `new` constructor, and `update_light_space`
//! stay in `shadows.rs`.

use super::shadows::ShadowRenderer;
use crate::scene::Scene;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

impl ShadowRenderer {
    pub fn render_static(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        gpu_meshes: &HashMap<u32, crate::render::GpuMesh>,
    ) {
        let mut render_resources = Vec::new();
        for entity in scene.iter() {
            if !entity.active || !entity.is_static {
                continue;
            }
            if entity.mesh.is_some() {
                if let Some(gpu_mesh) = gpu_meshes.get(&entity.id) {
                    let model_matrix = scene.compute_world_matrix(entity.id);
                    let model_arr = model_matrix.to_cols_array();
                    let entity_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Shadow Static Entity Uniform"),
                        contents: bytemuck::bytes_of(&model_arr),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Shadow Static Entity Bind Group"),
                        layout: &self.entity_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: entity_buf.as_entire_binding(),
                        }],
                    });

                    render_resources.push((entity.id, bind_group, gpu_mesh.num_indices));
                }
            }
        }

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

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);

            for (id, bind_group, num_indices) in &render_resources {
                if let Some(gpu_mesh) = gpu_meshes.get(id) {
                    render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        gpu_mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.set_bind_group(1, bind_group, &[]);
                    render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                }
            }
        }

        self.is_static_cached = true;
    }

    pub fn render_dynamic(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        gpu_meshes: &HashMap<u32, crate::render::GpuMesh>,
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

        let mut render_resources = Vec::new();
        for entity in scene.iter() {
            if !entity.active || entity.is_static {
                continue;
            }
            if entity.mesh.is_some() {
                if let Some(gpu_mesh) = gpu_meshes.get(&entity.id) {
                    let model_matrix = scene.compute_world_matrix(entity.id);
                    let model_arr = model_matrix.to_cols_array();
                    let entity_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Shadow Dynamic Entity Uniform"),
                        contents: bytemuck::bytes_of(&model_arr),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });

                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Shadow Dynamic Entity Bind Group"),
                        layout: &self.entity_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: entity_buf.as_entire_binding(),
                        }],
                    });

                    render_resources.push((entity.id, bind_group, gpu_mesh.num_indices));
                }
            }
        }

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

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);

            for (id, bind_group, num_indices) in &render_resources {
                if let Some(gpu_mesh) = gpu_meshes.get(id) {
                    render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        gpu_mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.set_bind_group(1, bind_group, &[]);
                    render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                }
            }
        }
    }
}
