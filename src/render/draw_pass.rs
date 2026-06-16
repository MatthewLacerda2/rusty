//! Shadow depth passes, dynamic global bind-group update, and the main scene
//! render pass. The per-camera stacking loop + post-FX live in `draw`.

use glam::Vec3;

use super::draw_resources::{Overlays, SolidResource};
use super::Renderer;
use crate::scene::{ClearFlags, LightType, Scene};

/// The framebuffer clear behavior for one camera in the stack (#93). Derived from the
/// camera's [`ClearFlags`] and whether it is the first (bottom) pass of the frame.
#[derive(Clone, Copy)]
pub(super) struct PassClear {
    /// `Some(backdrop)` clears color to that RGBA; `None` loads the existing color so
    /// this camera composites on top of what is already drawn (`DepthOnly`).
    pub color: Option<wgpu::Color>,
    /// The camera's clear flags, kept so the pass only redraws the skybox for a
    /// `Skybox` camera (a `DepthOnly` overlay must not paint over the world).
    pub flags: ClearFlags,
}

/// The dark editor/world backdrop the base camera clears to.
const BACKDROP: wgpu::Color = wgpu::Color {
    r: 0.06,
    g: 0.06,
    b: 0.08,
    a: 1.0,
};

impl PassClear {
    /// Resolve the clear ops for a pass. The bottom pass always clears color (nothing
    /// is under it); `DepthOnly` preserves color so a viewmodel/UI camera layers on
    /// top of the world. Every camera clears depth, so stacked geometry never clips
    /// against the layer below — the FPS viewmodel fix.
    pub(super) fn for_pass(is_first: bool, flags: ClearFlags) -> Self {
        let color = match flags {
            ClearFlags::Skybox | ClearFlags::SolidColor => Some(BACKDROP),
            // A DepthOnly bottom pass has nothing to composite over, so clear anyway.
            ClearFlags::DepthOnly if is_first => Some(BACKDROP),
            ClearFlags::DepthOnly => None,
        };
        Self { color, flags }
    }
}

/// Per-camera inputs threaded into the scene pass.
pub(super) struct ScenePassFrame {
    pub editor_mode: bool,
    pub clear: PassClear,
}

impl Renderer {
    pub(super) fn execute_scene_pass(
        &mut self,
        scene: &Scene,
        frame: ScenePassFrame,
        solid_render_resources: &[SolidResource],
        overlays: &Overlays,
    ) {
        let editor_mode = frame.editor_mode;
        let clear = frame.clear;
        let outline_resources = &overlays.outline;
        let grid_resources = &overlays.grid;
        let aabb_resources = &overlays.aabb;
        let axis_arrow_resources = &overlays.axis;
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
                    // Scene draws into the HDR offscreen target; the post-FX
                    // composite then writes the corrected image to `view_texture`.
                    // A `DepthOnly` camera loads the existing color (composites over
                    // the world); other cameras clear to the backdrop (#93).
                    view: &self.post_fx.scene_hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: match clear.color {
                            Some(c) => wgpu::LoadOp::Clear(c),
                            None => wgpu::LoadOp::Load,
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    // Every camera clears depth so its geometry sorts independently of
                    // the layer below — a viewmodel never clips through walls.
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

            // Render Skybox last (optimization). Only a Skybox camera paints it — a
            // DepthOnly/SolidColor overlay must not overwrite the world below it.
            if clear.flags == ClearFlags::Skybox {
                if let Some(skybox_tex) = &self.skybox_texture {
                    self.skybox_renderer.draw(
                        &mut render_pass,
                        &self.global_bind_group,
                        skybox_tex,
                    );
                }
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

        // 6. Submit the scene pass (it filled the HDR target + depth). Particles +
        // post-FX run from the per-camera loop in `draw`.
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
