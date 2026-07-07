//! Shadow depth passes, dynamic global bind-group update, and the main scene
//! render pass. The per-camera stacking loop + post-FX live in `draw`.

use glam::Vec3;

use crate::render::draw::resources::{OutlineResource, Overlays, SolidResource};
use crate::render::Renderer;
use crate::scene::{ClearFlags, LightType, Scene};

/// The framebuffer clear behavior for one camera in the stack (#93). Derived from the
/// camera's [`ClearFlags`] and whether it is the first (bottom) pass of the frame.
#[derive(Clone, Copy)]
pub(crate) struct PassClear {
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
    pub(crate) fn for_pass(is_first: bool, flags: ClearFlags) -> Self {
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
pub(crate) struct ScenePassFrame {
    pub editor_mode: bool,
    pub clear: PassClear,
}

impl Renderer {
    pub(crate) fn execute_scene_pass(
        &mut self,
        scene: &Scene,
        frame: ScenePassFrame,
        solid_render_resources: &[SolidResource],
        overlays: &Overlays,
    ) {
        // 4. Render Pass Setup
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Scene Render Encoder"),
            });

        // A. Shadow depth sweep passes
        self.run_shadow_passes(&mut encoder, scene);
        // Bind active skybox view & sampler into the global group for reflections.
        self.update_global_bind_group();
        // B. Main scene + overlay pass.
        self.record_scene_pass(&mut encoder, &frame, solid_render_resources, overlays);

        // 6. Submit the scene pass (it filled the HDR target + depth). Particles +
        // post-FX run from the per-camera loop in `draw`.
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Record the main scene pass (solids, outline, skybox, overlays) into `encoder`.
    fn record_scene_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &ScenePassFrame,
        solid_render_resources: &[SolidResource],
        overlays: &Overlays,
    ) {
        let editor_mode = frame.editor_mode;
        let clear = frame.clear;
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Scene Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                // Scene draws into the HDR offscreen target (post-FX composites later).
                // A `DepthOnly` camera loads existing color; others clear backdrop (#93).
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
                // Every camera clears depth so its geometry sorts independently of the
                // layer below — a viewmodel never clips through walls.
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
        // Solid entities, then the editor selection outline.
        self.draw_solids(&mut render_pass, solid_render_resources);
        if editor_mode {
            self.draw_outline(&mut render_pass, &overlays.outline);
        }

        // Render Skybox last (optimization). Only a Skybox camera paints it — a
        // DepthOnly/SolidColor overlay must not overwrite the world below it. With a
        // panorama bound we draw it; with none, a Skybox camera falls back to the
        // procedural sky->horizon->ground gradient (#256) instead of the flat
        // backdrop, so an empty scene reads as a lit environment (Unity's default-
        // skybox fallback). Both paint only the far-plane pixels no geometry covered.
        if clear.flags == ClearFlags::Skybox {
            match &self.skybox_texture {
                Some(skybox_tex) => {
                    self.skybox_renderer
                        .draw(&mut render_pass, &self.global_bind_group, skybox_tex)
                }
                None => self
                    .skybox_renderer
                    .draw_gradient(&mut render_pass, &self.global_bind_group),
            }
        }

        // 5. Render debug overlay tools
        render_pass.set_pipeline(&self.line_pipeline);
        if editor_mode {
            let o = overlays;
            self.draw_editor_overlays(&mut render_pass, &o.grid, &o.aabb, &o.axis);
            // Light- & reflection-probe gizmos (#284): same line pipeline, same
            // editor-only gate. Visualization only — never drawn in a game render.
            self.draw_probe_overlays(&mut render_pass, &o.probes);
        }
    }

    /// Pick the directional caster, update light space, run static+dynamic shadow sweeps.
    fn run_shadow_passes(&mut self, encoder: &mut wgpu::CommandEncoder, scene: &Scene) {
        let mut dir_light_dir = Vec3::new(-0.5, -1.0, -0.3).normalize();
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            if light.light_type == LightType::Directional {
                let transform = scene.world.transform(id).expect("mandatory Transform");
                dir_light_dir = (transform.rotation * Vec3::NEG_Z).normalize();
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
            self.shadow_renderer.render_static(
                &self.device,
                &self.queue,
                encoder,
                scene,
                &self.gpu_meshes,
            );
        }
        self.shadow_renderer.render_dynamic(
            &self.device,
            &self.queue,
            encoder,
            scene,
            &self.gpu_meshes,
        );
    }

    /// Rebuild the group-0 bind group with the active skybox, but only when the skybox
    /// changed — its camera/lighting buffers are persistent, so an unchanged skybox
    /// needs no rebuild (skips ~one bind-group creation per camera per frame, #210).
    fn update_global_bind_group(&mut self) {
        if !self.global_bind_group_dirty {
            return;
        }
        self.global_bind_group_dirty = false;
        let skybox = self.skybox_texture.as_deref();
        let skybox_view = skybox.map_or(&self.default_texture.view, |t| &t.view);
        let skybox_sampler = skybox.map_or(&self.default_texture.sampler, |t| &t.sampler);
        // The active reflection probe's cube at binding 4, else the black fallback cube —
        // the shader picks skybox vs cube via the `refl_has_cubemap` flag, so a fallback
        // here is never sampled but keeps the bind group valid against the layout.
        let cube = self.reflection_cube.as_ref().unwrap_or(&self.default_cube);
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&cube.view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&cube.sampler),
                },
            ],
        });
    }

    fn draw_solids<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        solid_render_resources: &'a [SolidResource],
    ) {
        // Groups 1 (entity) + 2 (material) are the pool's persistent bind groups (#210).
        let pool = self.entity_pool.as_ref().expect("entity pool present");
        for (id, mesh_id, num_indices) in solid_render_resources {
            let (Some(gpu_mesh), Some(entity_bg), Some(material_bg)) = (
                self.gpu_meshes.get(mesh_id),
                pool.entity_bind_group(*id),
                pool.material_bind_group(*id),
            ) else {
                continue;
            };
            render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_bind_group(1, entity_bg, &[]);
            render_pass.set_bind_group(2, material_bg, &[]);
            render_pass.draw_indexed(0..*num_indices, 0, 0..1);
        }
    }

    fn draw_outline<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        outline_resources: &'a Option<OutlineResource>,
    ) {
        let Some((
            _selected_id,
            outline_mesh_id,
            _outline_ent_buf,
            outline_bind_group,
            num_indices,
        )) = outline_resources
        else {
            return;
        };
        let Some(gpu_mesh) = self.gpu_meshes.get(outline_mesh_id) else {
            return;
        };
        // The outline is a flat unlit silhouette (`use_texture = 0`), so group(2) is
        // never sampled — bind the default material group to satisfy the layout.
        render_pass.set_pipeline(&self.outline_pipeline);
        render_pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(gpu_mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.set_bind_group(1, outline_bind_group, &[]);
        render_pass.set_bind_group(2, &self.default_material_bind_group, &[]);
        render_pass.draw_indexed(0..*num_indices, 0, 0..1);
    }
}
