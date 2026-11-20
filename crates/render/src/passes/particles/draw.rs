//! crates/render/src/passes/particles/draw.rs — per-frame sprite particle draws.
//!
//! Turns every visible sprite emitter into a run of instances (a GPU-simulated one
//! into one draw of its buffer, #2170, `runs`), orders the emitters
//! back to front (so smoke in front of fire composites over it), merges neighbours
//! that share a blend + texture into one draw, uploads the instances + globals, and
//! records a pass into the HDR scene target (after solids/skybox/transparents,
//! before post-FX). Mesh emitters are skipped here — they draw with the solids.

use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use std::rc::Rc;

use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use super::instance::{Eye, ParticleGlobals, ParticleInstance};
use super::runs::{collect_runs, EmitterRun, Sprites};
use crate::gpu::uniforms::FogUniform;
use crate::postfx::effects::taa;
use crate::timing::GpuPass;
use crate::{GpuTexture, RenderView, Renderer};
use rusty_components::particle::ParticleBlend;
use rusty_scene::Camera;
use rusty_scene::{Scene, SceneId};

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

/// A draw call: a slice of the frame's instance buffer sharing blend + texture, or
/// one GPU-simulated emitter's slots.
struct ParticleBatch {
    blend: ParticleBlend,
    texture: Rc<GpuTexture>,
    instances: std::ops::Range<u32>,
    /// The GPU-simulated emitter it draws, by key, else the instance buffer.
    gpu: Option<(SceneId, u32)>,
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
        view_proj: Mat4,
    ) -> ParticleDraws {
        let eye = Eye {
            pos: camera.position,
            fwd: camera.forward(),
        };
        let runs = collect_runs(scene, camera.culling_mask, eye, &self.particle_renderer.gpu);
        if runs.is_empty() {
            return ParticleDraws::default();
        }
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
        self.queue.counted_write_buffer(
            &self.particle_renderer.globals_buffer,
            0,
            bytemuck::bytes_of(&globals),
        );

        for texture in runs.iter().filter_map(|r| r.texture.as_ref()) {
            self.note_map(scene.id(), texture);
        }
        let (all, batches, drawn) = self.batch_runs(runs);
        // Only `Gpu` emitters: no instance is built, and no buffer either.
        let instance_buffer = (!all.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Instances"),
                    contents: bytemuck::cast_slice(&all),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        self.ensure_scene_depth_bind_group(view);
        self.encode_particle_pass(view, &batches, instance_buffer.as_ref());
        ParticleDraws {
            draw_calls: batches.len() as u32,
            instances: drawn,
        }
    }

    /// Build the view's cached scene-depth bind group if absent (first frame that
    /// needs it, or after the view's resize invalidated it). The depth view only
    /// changes on resize, so it is reused across frames and cameras (#210).
    pub(crate) fn ensure_scene_depth_bind_group(&self, view: &mut RenderView) {
        if view.scene_depth_bind_group.is_some() {
            return;
        }
        view.scene_depth_bind_group =
            Some(self.device.counted_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Scene Depth Bind Group"),
                layout: &self.scene_depth_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view.depth_view),
                }],
            }));
    }

    /// Pack the (already back-to-front) runs into one instance array, merging
    /// neighbouring runs with the same blend + texture into one draw; a
    /// GPU-simulated emitter is a draw of its own. Also the particles drawn.
    fn batch_runs(
        &mut self,
        runs: Vec<EmitterRun>,
    ) -> (Vec<ParticleInstance>, Vec<ParticleBatch>, u32) {
        let mut all: Vec<ParticleInstance> = Vec::new();
        let mut batches: Vec<ParticleBatch> = Vec::new();
        let mut drawn = 0;
        for run in runs {
            let texture = match &run.texture {
                Some(path) => self.load_texture(path),
                None => Rc::clone(&self.particle_renderer.default_sprite),
            };
            let instances = match run.sprites {
                Sprites::Gpu(key, slots, live) => {
                    drawn += live;
                    batches.push(ParticleBatch {
                        blend: run.blend,
                        texture,
                        instances: 0..slots,
                        gpu: Some(key),
                    });
                    continue;
                }
                Sprites::Instances(instances) => instances,
            };
            let start = all.len() as u32;
            all.extend_from_slice(&instances);
            let end = all.len() as u32;
            drawn += end - start;
            match batches.last_mut() {
                Some(last)
                    if last.gpu.is_none()
                        && last.blend == run.blend
                        && Rc::ptr_eq(&last.texture, &texture) =>
                {
                    last.instances.end = end;
                }
                _ => batches.push(ParticleBatch {
                    blend: run.blend,
                    texture,
                    instances: start..end,
                    gpu: None,
                }),
            }
        }
        (all, batches, drawn)
    }

    /// Record the particle render pass: load the HDR colour, bind the scene depth
    /// read-only (tested against, and sampled for soft particles), then draw each
    /// batch with its blend pipeline and sprite texture.
    fn encode_particle_pass(
        &self,
        view: &RenderView,
        batches: &[ParticleBatch],
        instance_buffer: Option<&wgpu::Buffer>,
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
        let reactive = view.post_fx.taa_reactive().is_some();
        {
            let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Particle Pass"),
                color_attachments: &taa::reactive::hdr_attachments(view),
                // `depth_ops: None` makes the attachment read-only, which is what lets
                // the same texture be sampled (group 2) in this pass.
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: self.gpu_timer.writes(GpuPass::Particles),
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &pr.globals_bind_group, &[]);
            pass.set_bind_group(2, depth_bg, &[]);
            pass.set_bind_group(3, &self.global_bind_group, &[]);
            pass.set_index_buffer(pr.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            if let Some(instances) = instance_buffer {
                pass.set_vertex_buffer(0, instances.slice(..));
            }
            for batch in batches {
                pass.set_bind_group(1, &batch.texture.bind_group, &[]);
                let group = batch.gpu.and_then(|key| pr.gpu.emitters.get(&key));
                match group.and_then(|e| e.groups.as_ref()) {
                    Some((_, draw)) => {
                        pass.set_pipeline(pr.gpu_pipeline_for(batch.blend, reactive));
                        pass.set_bind_group(0, draw, &[]);
                        pass.draw_indexed(0..6, 0, batch.instances.clone());
                        pass.set_bind_group(0, &pr.globals_bind_group, &[]);
                    }
                    None if batch.gpu.is_some() => {}
                    None => {
                        pass.set_pipeline(pr.pipeline_for(batch.blend, reactive));
                        pass.draw_indexed(0..6, 0, batch.instances.clone());
                    }
                }
            }
        }
        self.queue.counted_submit(std::iter::once(encoder.finish()));
    }
}

#[cfg(test)]
#[path = "sprite_gpu_tests.rs"]
mod sprite_gpu_tests;
