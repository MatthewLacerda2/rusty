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

use crate::render::passes::decals::{Decal, DecalGlobals, DecalUniform};
use crate::render::{Camera, GpuTexture, Renderer};
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
    pub(crate) fn draw_decals(&mut self, scene: &Scene, camera: &Camera) {
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
    fn encode_decal_pass(&mut self, draws: &[DecalDraw]) {
        // Bind the scene depth as a sampled texture for surface reconstruction. The
        // depth view only changes on resize, so this bind group is cached and reused
        // across frames/cameras instead of rebuilt every call (#210); `resize`
        // invalidates it.
        self.ensure_decal_depth_bind_group();
        let dr = &self.decal_renderer;
        let depth_bg = self
            .decal_depth_bind_group
            .as_ref()
            .expect("decal depth bind group built");

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
            pass.set_bind_group(2, depth_bg, &[]);
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

    /// Build the cached decal-pass depth bind group if absent (first decal frame or
    /// after a resize invalidated it).
    fn ensure_decal_depth_bind_group(&mut self) {
        if self.decal_depth_bind_group.is_some() {
            return;
        }
        self.decal_depth_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Decal Depth Bind Group"),
                layout: &self.decal_renderer.depth_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.depth_view),
                }],
            }));
    }
}

/// Vertex layout for the decal cube: just a local-space position.
pub(crate) fn decal_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];
    wgpu::VertexBufferLayout {
        array_stride: (3 * std::mem::size_of::<f32>()) as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

/// Unit cube spanning `[-0.5, 0.5]³` as 8 corners + 36 CCW indices.
pub(crate) fn unit_cube() -> ([[f32; 3]; 8], [u16; 36]) {
    let verts = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    let indices = [
        0u16, 1, 2, 0, 2, 3, // -Z
        4, 6, 5, 4, 7, 6, // +Z
        0, 4, 5, 0, 5, 1, // -Y
        3, 2, 6, 3, 6, 7, // +Y
        0, 3, 7, 0, 7, 4, // -X
        1, 5, 6, 1, 6, 2, // +X
    ];
    (verts, indices)
}
