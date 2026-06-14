//! Shadow depth passes, dynamic global bind-group update, and the main scene
//! render pass. Extracted verbatim from the original `Renderer::render`.

use glam::Vec3;

use super::draw_resources::{
    AabbResource, AxisResource, GridResource, OutlineResource, SolidResource,
};
use super::Renderer;
use crate::core::scene::{LightType, Scene};

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn execute_scene_pass(
        &mut self,
        scene: &Scene,
        view_texture: &wgpu::TextureView,
        editor_mode: bool,
        solid_render_resources: &[SolidResource],
        outline_resources: &Option<OutlineResource>,
        grid_resources: &Option<GridResource>,
        aabb_resources: &[AabbResource],
        axis_arrow_resources: &[AxisResource],
    ) {
        // 4. Render Pass Setup
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Render Encoder"),
            });

        // A. Shadow depth sweep passes
        let mut dir_light_dir = Vec3::new(-0.5, -1.0, -0.3).normalize();
        for entity in scene.iter() {
            if entity.active {
                if let Some(light) = &entity.light {
                    if light.light_type == LightType::Directional {
                        dir_light_dir = (entity.transform.rotation * Vec3::NEG_Z).normalize();
                    }
                }
            }
        }
        self.shadow_renderer
            .update_light_space(&self.queue, dir_light_dir);
        self.queue.write_buffer(
            &self.shadow_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.shadow_renderer.light_space_matrix.to_cols_array()),
        );

        if !self.shadow_renderer.is_static_cached {
            self.shadow_renderer
                .render_static(&self.device, &mut encoder, scene, &self.gpu_meshes);
        }
        self.shadow_renderer
            .render_dynamic(&self.device, &mut encoder, scene, &self.gpu_meshes);

        // Dynamic global bind group updates to bind active skybox view & sampler for reflections
        let skybox_view = self
            .skybox_texture
            .as_ref()
            .map(|tex| &tex.view)
            .unwrap_or(&self.default_texture.view);

        let skybox_sampler = self
            .skybox_texture
            .as_ref()
            .map(|tex| &tex.sampler)
            .unwrap_or(&self.default_texture.sampler);

        self.global_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Global Bind Group with Reflections"),
            layout: &self.camera_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(skybox_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(skybox_sampler),
                },
            ],
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: view_texture,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06, // Sleek modern dark backdrop
                            g: 0.06,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set global bindings
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.global_bind_group, &[]);
            render_pass.set_bind_group(3, &self.shadow_bind_group, &[]);

            // Render Solid Entities
            for (id, _ent_buf, _bones_buf, bind_group, tex, num_indices) in solid_render_resources {
                if let Some(gpu_mesh) = self.gpu_meshes.get(id) {
                    render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(
                        gpu_mesh.index_buffer.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.set_bind_group(1, bind_group, &[]);
                    render_pass.set_bind_group(2, &tex.bind_group, &[]);
                    render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                }
            }

            // Render Selection Outline Silhouette (if in editor mode)
            if editor_mode {
                if let Some((
                    selected_id,
                    _outline_ent_buf,
                    _outline_bones_buf,
                    outline_bind_group,
                    num_indices,
                )) = outline_resources
                {
                    if let Some(gpu_mesh) = self.gpu_meshes.get(selected_id) {
                        let tex = solid_render_resources
                            .iter()
                            .find(|(id, _, _, _, _, _)| id == selected_id)
                            .map(|(_, _, _, _, tex, _)| tex)
                            .unwrap_or(&self.default_texture);

                        render_pass.set_pipeline(&self.outline_pipeline);
                        render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(
                            gpu_mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.set_bind_group(1, outline_bind_group, &[]);
                        render_pass.set_bind_group(2, &tex.bind_group, &[]);
                        render_pass.draw_indexed(0..*num_indices, 0, 0..1);
                    }
                }
            }

            // Render Skybox last (optimization)
            if let Some(skybox_tex) = &self.skybox_texture {
                self.skybox_renderer
                    .draw(&mut render_pass, &self.global_bind_group, skybox_tex);
            }

            // 5. Render debug overlay tools
            render_pass.set_pipeline(&self.line_pipeline);

            // A. Draw floor grid (in EditorMode only)
            if editor_mode {
                if let Some((_grid_buf_unif, _default_bones_buf, grid_bind_group)) = grid_resources
                {
                    if let Some(grid_buf) = &self.grid_vertex_buffer {
                        render_pass.set_vertex_buffer(0, grid_buf.slice(..));
                        render_pass.set_bind_group(1, grid_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.default_texture.bind_group, &[]);
                        render_pass.draw(0..self.grid_count, 0..1);
                    }
                }

                // B. Draw AABB outlines for active colliders
                for (aabb_wire_buffer, _entity_buf, _default_bones_buf, col_bind_group) in
                    aabb_resources
                {
                    render_pass.set_vertex_buffer(0, aabb_wire_buffer.slice(..));
                    render_pass.set_bind_group(1, col_bind_group, &[]);
                    render_pass.draw(0..24, 0..1);
                }

                // C. Draw global axis arrows overlay for the selected entity
                for (i, _entity_buf, _default_bones_buf, bind_group) in axis_arrow_resources {
                    let buffer = match i {
                        0 => &self.axis_x_buffer,
                        1 => &self.axis_y_buffer,
                        2 => &self.axis_z_buffer,
                        _ => &None,
                    };
                    if let Some(buf) = buffer {
                        render_pass.set_vertex_buffer(0, buf.slice(..));
                        render_pass.set_bind_group(1, bind_group, &[]);
                        render_pass.draw(0..self.axis_count, 0..1);
                    }
                }
            }
        } // End of Render Pass

        // 6. Submit Render commands
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
