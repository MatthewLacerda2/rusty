//! src/render/decals_draw.rs — per-frame box-projector decal draw orchestration.
//!
//! Reads the scene's runtime decal registry, uploads camera globals + one uniform
//! per decal, and records a render pass that draws each projector box into the HDR
//! scene target (after solids/skybox, before particles + post-FX). For each box the
//! shader reconstructs the underlying surface from the bound scene depth and
//! projects the decal texture onto it. The GPU resources + pipeline live in
//! `decals.rs`.

use std::rc::Rc;

use wgpu::util::DeviceExt;

use super::decals::{Decal, DecalGlobals, DecalUniform};
use super::{Camera, GpuTexture, Renderer};
use crate::scene::Scene;

/// One decal resolved for drawing: its uniform bind group + sprite texture.
struct DecalDraw {
    uniform_bg: wgpu::BindGroup,
    texture: Rc<GpuTexture>,
    // Keep the per-decal uniform buffer alive until the pass is submitted.
    _buffer: wgpu::Buffer,
}

impl Renderer {
    /// Draw the scene's decals into the HDR target. Called from the scene pass
    /// after solids/skybox so decals overlay the lit surfaces, and before the
    /// particle pass + post-FX chain.
    pub(super) fn draw_decals(&mut self, scene: &Scene, camera: &Camera) {
        if scene.decals.is_empty() {
            return;
        }

        let aspect = self.size.width as f32 / self.size.height as f32;
        let view_proj = camera.build_view_projection(aspect);
        let inv_view_proj = view_proj.inverse();
        let globals = DecalGlobals {
            view_proj: view_proj.to_cols_array(),
            inv_view_proj: inv_view_proj.to_cols_array(),
            camera_pos: [camera.position.x, camera.position.y, camera.position.z, 0.0],
        };
        self.queue.write_buffer(
            &self.decal_renderer.globals_buffer,
            0,
            bytemuck::bytes_of(&globals),
        );

        // Pre-load every referenced decal texture (mutably borrows self), then
        // build the per-decal bind groups in a second pass.
        let textures: Vec<Rc<GpuTexture>> = scene
            .decals
            .iter()
            .map(|d| match &d.texture {
                Some(path) => self.load_texture(path),
                None => Rc::clone(&self.default_texture),
            })
            .collect();

        let draws: Vec<DecalDraw> = scene
            .decals
            .iter()
            .zip(textures)
            .map(|(decal, texture)| self.build_decal_draw(decal, texture))
            .collect();

        self.encode_decal_pass(&draws);
    }

    /// Build the per-decal uniform buffer + its bind group.
    fn build_decal_draw(&self, decal: &Decal, texture: Rc<GpuTexture>) -> DecalDraw {
        let model = decal.model_matrix();
        let uniform = DecalUniform {
            model: model.to_cols_array(),
            inv_model: model.inverse().to_cols_array(),
            color: decal.color,
        };
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Decal Uniform"),
                contents: bytemuck::bytes_of(&uniform),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let uniform_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Decal Uniform Bind Group"),
            layout: &self.decal_renderer.decal_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        DecalDraw {
            uniform_bg,
            texture,
            _buffer: buffer,
        }
    }

    /// Record the decal render pass: load the existing HDR colour (no depth
    /// attachment — depth is bound as a sampled texture), then draw each projector
    /// box. The shader reconstructs the surface and projects the decal onto it.
    fn encode_decal_pass(&self, draws: &[DecalDraw]) {
        let dr = &self.decal_renderer;

        // Bind the scene depth as a sampled texture for surface reconstruction.
        let depth_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Decal Depth Bind Group"),
            layout: &dr.depth_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&self.depth_view),
            }],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Decal Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Decal Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.post_fx.scene_hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&dr.pipeline);
            pass.set_bind_group(0, &dr.globals_bind_group, &[]);
            pass.set_bind_group(2, &depth_bg, &[]);
            pass.set_vertex_buffer(0, dr.vertex_buffer.slice(..));
            pass.set_index_buffer(dr.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            for draw in draws {
                pass.set_bind_group(1, &draw.uniform_bg, &[]);
                pass.set_bind_group(3, &draw.texture.bind_group, &[]);
                pass.draw_indexed(0..36, 0, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
