//! src/render/passes/particles/draw.rs — per-frame sprite particle draws.
//!
//! Turns every visible sprite emitter into a run of instances, orders the emitters
//! back to front (so smoke in front of fire composites over it), merges neighbours
//! that share a blend + texture into one draw, uploads the instances + globals, and
//! records a pass into the HDR scene target (after solids/skybox/transparents,
//! before post-FX). Mesh emitters are skipped here — they draw with the solids.

use std::rc::Rc;

use glam::Vec3;
use wgpu::util::DeviceExt;

use super::instance::{emitter_instances, emitter_light, Eye, ParticleGlobals, ParticleInstance};
use crate::components::particle::ParticleBlend;
use crate::components::ParticleRenderMode;
use crate::render::draw::sort::back_to_front;
use crate::render::gpu::uniforms::FogUniform;
use crate::render::{GpuTexture, RenderView, Renderer};
use crate::scene::Camera;
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

/// One emitter's sprites, before textures are resolved.
struct EmitterRun {
    blend: ParticleBlend,
    texture: Option<String>,
    instances: Vec<ParticleInstance>,
}

/// A draw call: a slice of the frame's instance buffer sharing blend + texture.
struct ParticleBatch {
    blend: ParticleBlend,
    texture: Rc<GpuTexture>,
    instances: std::ops::Range<u32>,
}

/// What the sprite pass drew for one camera, for the frame counters (#433).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ParticleDraws {
    pub draw_calls: u32,
    pub instances: u32,
}

impl Renderer {
    /// Draw every sprite emitter's live particles into the HDR scene target.
    pub(crate) fn draw_particles(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
    ) -> ParticleDraws {
        let eye = Eye {
            pos: camera.position,
            fwd: camera.forward(),
        };
        let runs = collect_runs(scene, camera.culling_mask, eye);
        if runs.is_empty() {
            return ParticleDraws::default();
        }
        let view_proj = camera.build_view_projection(view.aspect());
        let (right, up) = billboard_basis(camera);
        let globals = ParticleGlobals {
            view_proj: view_proj.to_cols_array(),
            inv_view_proj: view_proj.inverse().to_cols_array(),
            cam_right: right.extend(0.0).to_array(),
            cam_up: up.extend(0.0).to_array(),
            cam_pos: eye.pos.extend(0.0).to_array(),
            cam_fwd: eye.fwd.extend(0.0).to_array(),
            fog: FogUniform::from_settings(&scene.fog),
        };
        self.queue.write_buffer(
            &self.particle_renderer.globals_buffer,
            0,
            bytemuck::bytes_of(&globals),
        );

        let (all, batches) = self.batch_runs(runs);
        let instance_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Particle Instances"),
                contents: bytemuck::cast_slice(&all),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.ensure_scene_depth_bind_group(view);
        self.encode_particle_pass(view, &batches, &instance_buffer);
        ParticleDraws {
            draw_calls: batches.len() as u32,
            instances: all.len() as u32,
        }
    }

    /// Pack the (already back-to-front) runs into one instance array, merging
    /// neighbouring runs with the same blend + texture into one draw.
    fn batch_runs(&mut self, runs: Vec<EmitterRun>) -> (Vec<ParticleInstance>, Vec<ParticleBatch>) {
        let mut all: Vec<ParticleInstance> = Vec::new();
        let mut batches: Vec<ParticleBatch> = Vec::new();
        for run in runs {
            let texture = match &run.texture {
                Some(path) => self.load_texture(path),
                None => Rc::clone(&self.default_texture),
            };
            let start = all.len() as u32;
            all.extend_from_slice(&run.instances);
            let end = all.len() as u32;
            match batches.last_mut() {
                Some(last) if last.blend == run.blend && Rc::ptr_eq(&last.texture, &texture) => {
                    last.instances.end = end;
                }
                _ => batches.push(ParticleBatch {
                    blend: run.blend,
                    texture,
                    instances: start..end,
                }),
            }
        }
        (all, batches)
    }

    /// Record the particle render pass: load the HDR colour, bind the scene depth
    /// read-only (tested against, and sampled for soft particles), then draw each
    /// batch with its blend pipeline and sprite texture.
    fn encode_particle_pass(
        &self,
        view: &RenderView,
        batches: &[ParticleBatch],
        instance_buffer: &wgpu::Buffer,
    ) {
        let pr = &self.particle_renderer;
        let depth_bg = view
            .scene_depth_bind_group
            .as_ref()
            .expect("scene depth bind group built");
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Particle Encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Particle Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &view.post_fx.scene_hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                // `depth_ops: None` makes the attachment read-only, which is what lets
                // the same texture be sampled (group 2) in this pass.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &pr.globals_bind_group, &[]);
            pass.set_bind_group(2, depth_bg, &[]);
            pass.set_bind_group(3, &self.global_bind_group, &[]);
            pass.set_index_buffer(pr.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.set_vertex_buffer(0, instance_buffer.slice(..));
            for batch in batches {
                pass.set_pipeline(pr.pipeline_for(batch.blend));
                pass.set_bind_group(1, &batch.texture.bind_group, &[]);
                pass.draw_indexed(0..6, 0, batch.instances.clone());
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}

/// Every active, visible sprite emitter's instances, farthest emitter first.
fn collect_runs(scene: &Scene, culling_mask: u32, eye: Eye) -> Vec<EmitterRun> {
    let mut runs: Vec<(EmitterRun, f32)> = Vec::new();
    for id in scene.world.ids_with_particles() {
        // Inactive emitters, and those on a layer the camera culls (#92), don't draw.
        if !scene.world.is_active(id)
            || !crate::scene::layer_in_mask(scene.world.layer(id), culling_mask)
        {
            continue;
        }
        let emitter = scene
            .world
            .particles(id)
            .expect("id came from ids_with_particles");
        if emitter.runtime.particles.is_empty() || emitter.render.mode == ParticleRenderMode::Mesh {
            continue;
        }
        let origin = scene.world_matrix(id).w_axis.truncate();
        let light = emitter_light(scene, &emitter, origin);
        let (instances, depth) = emitter_instances(&emitter, light, eye);
        let run = EmitterRun {
            blend: emitter.blend,
            texture: emitter.texture.clone(),
            instances,
        };
        runs.push((run, depth));
    }
    back_to_front(&mut runs);
    runs.into_iter().map(|(run, _)| run).collect()
}
