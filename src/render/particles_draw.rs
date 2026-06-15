//! src/render/particles_draw.rs — per-frame billboard particle draw orchestration.
//!
//! Reads each emitter's live particles, batches them by (blend, sprite texture),
//! uploads instances + billboard globals, and records a render pass that draws into
//! the HDR scene target (after solids/skybox, before post-FX). The GPU resources +
//! pipelines it uses live in `particles.rs`.

use std::rc::Rc;

use glam::Vec3;
use wgpu::util::DeviceExt;

use super::particles::{ParticleGlobals, ParticleInstance};
use super::{Camera, GpuTexture, Renderer};
use crate::components::particle::ParticleBlend;
use crate::scene::Scene;

/// Camera-facing billboard basis (right, up). Derived from the camera forward, with
/// a fallback when the camera looks near straight up/down — there `forward × Y`
/// collapses, so `camera.right()` would be NaN/zero and every quad would vanish.
fn billboard_basis(camera: &Camera) -> (Vec3, Vec3) {
    let forward = camera.forward();
    // Pick a reference up that isn't parallel to forward.
    let reference = if forward.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let right = forward.cross(reference).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    (right, up)
}

/// A batch of instances sharing a blend mode + sprite texture, ready to draw.
struct ParticleBatch {
    blend: ParticleBlend,
    texture: Rc<GpuTexture>,
    instances: Vec<ParticleInstance>,
}

impl Renderer {
    /// Draw every emitter's live particles into the HDR scene target. Called from
    /// the scene pass after solids/skybox, before the post-FX chain runs.
    pub(super) fn draw_particles(&mut self, scene: &Scene, camera: &Camera) {
        let batches = self.collect_particle_batches(scene);
        if batches.is_empty() {
            return;
        }

        let aspect = self.size.width as f32 / self.size.height as f32;
        let view_proj = camera.build_view_projection(aspect);
        let (right, up) = billboard_basis(camera);
        let globals = ParticleGlobals {
            view_proj: view_proj.to_cols_array(),
            cam_right: [right.x, right.y, right.z, 0.0],
            cam_up: [up.x, up.y, up.z, 0.0],
        };
        self.queue.write_buffer(
            &self.particle_renderer.globals_buffer,
            0,
            bytemuck::bytes_of(&globals),
        );

        // Upload all instances into one buffer; each batch draws its own slice.
        let mut all = Vec::new();
        let mut ranges = Vec::new();
        for batch in &batches {
            let start = all.len() as u32;
            all.extend_from_slice(&batch.instances);
            ranges.push((start, batch.instances.len() as u32));
        }
        let instance_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle Instances"),
                contents: bytemuck::cast_slice(&all),
                usage: wgpu::BufferUsages::VERTEX,
            });

        self.encode_particle_pass(&batches, &ranges, &instance_buffer);
    }

    /// Record the particle render pass: load the existing HDR colour + depth, then
    /// draw each batch with its blend pipeline and sprite texture.
    fn encode_particle_pass(
        &self,
        batches: &[ParticleBatch],
        ranges: &[(u32, u32)],
        instance_buffer: &wgpu::Buffer,
    ) {
        let pr = &self.particle_renderer;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Particle Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.post_fx.scene_hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &pr.globals_bind_group, &[]);
            pass.set_index_buffer(pr.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.set_vertex_buffer(0, instance_buffer.slice(..));
            for (batch, &(start, count)) in batches.iter().zip(ranges) {
                if count == 0 {
                    continue;
                }
                pass.set_pipeline(pr.pipeline_for(batch.blend));
                pass.set_bind_group(1, &batch.texture.bind_group, &[]);
                pass.draw_indexed(0..6, 0, start..(start + count));
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Gather live particles from every active emitter into per-(blend,texture)
    /// batches, resolving each emitter's sprite (or the default checker texture).
    fn collect_particle_batches(&mut self, scene: &Scene) -> Vec<ParticleBatch> {
        // Pre-load every referenced sprite texture (mutably borrows self), then
        // build the batches in a second pass.
        let mut specs: Vec<(ParticleBlend, Option<String>, Vec<ParticleInstance>)> = Vec::new();
        for entity in scene.iter() {
            if !entity.active {
                continue;
            }
            let Some(emitter) = &entity.particles else {
                continue;
            };
            if emitter.runtime.particles.is_empty() {
                continue;
            }
            let instances: Vec<ParticleInstance> = emitter
                .runtime
                .particles
                .iter()
                .map(|p| ParticleInstance {
                    center: p.position.to_array(),
                    size: p.current_size(),
                    color: p.current_color(),
                })
                .collect();
            specs.push((emitter.blend, emitter.texture.clone(), instances));
        }

        specs
            .into_iter()
            .map(|(blend, tex_path, instances)| {
                let texture = match tex_path {
                    Some(path) => self.load_texture(&path),
                    None => Rc::clone(&self.default_texture),
                };
                ParticleBatch {
                    blend,
                    texture,
                    instances,
                }
            })
            .collect()
    }
}
