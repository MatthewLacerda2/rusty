//! The billboard basis falls back when the camera looks straight up or down: forward × Y collapses
//! and every quad would vanish. `depth_ops: None` makes the depth read-only so the same texture
//! can be sampled in this pass.

use crate::gpu::groups;
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

fn billboard_basis(camera: &Camera) -> (Vec3, Vec3) {
    let forward = camera.forward();
    // Within about 8° of straight up or down, Z is the reference axis.
    let reference = if forward.y.abs() > 0.99 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let right = forward.cross(reference).normalize_or_zero();
    let up = right.cross(forward).normalize_or_zero();
    (right, up)
}

struct ParticleBatch {
    blend: ParticleBlend,
    texture: Rc<GpuTexture>,
    instances: std::ops::Range<u32>,
    gpu: Option<(SceneId, u32)>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ParticleDraws {
    pub draw_calls: u32,
    pub instances: u32,
}

impl Renderer {
    pub(crate) fn draw_particles(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
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

        // Due renders only: an emitter has no change signal, and a map new to the cache makes the
        // next render due.
        if self.notes_due() {
            for texture in runs.iter().filter_map(|r| r.texture.as_ref()) {
                self.note_map(scene.id(), texture.as_str());
            }
        }
        let (all, batches, drawn) = self.batch_runs(runs);
        let instance_buffer = (!all.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Instances"),
                    contents: bytemuck::cast_slice(&all),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        self.ensure_scene_depth_bind_group(view);
        self.encode_particle_pass(encoder, view, &batches, instance_buffer.as_ref());
        ParticleDraws {
            draw_calls: batches.len() as u32,
            instances: drawn,
        }
    }

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

    fn encode_particle_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &RenderView,
        batches: &[ParticleBatch],
        instance_buffer: Option<&wgpu::Buffer>,
    ) {
        let pr = &self.particle_renderer;
        let depth_bg = view
            .scene_depth_bind_group
            .as_ref()
            .expect("scene depth bind group built");
        let reactive = view.post_fx.taa_reactive().is_some();
        {
            let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Particle Pass"),
                color_attachments: &taa::reactive::hdr_attachments(view),
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view.depth_view,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: self.gpu_timer.writes(GpuPass::Particles),
                occlusion_query_set: None,
            });

            pass.set_bind_group(groups::DRAW, &pr.globals_bind_group, &[]);
            pass.set_bind_group(groups::SCENE, depth_bg, &[]);
            pass.set_bind_group(groups::GLOBAL, &self.global_bind_group, &[]);
            pass.set_index_buffer(pr.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            if let Some(instances) = instance_buffer {
                pass.set_vertex_buffer(0, instances.slice(..));
            }
            for batch in batches {
                pass.set_bind_group(groups::MATERIAL, &batch.texture.bind_group, &[]);
                let group = batch.gpu.and_then(|key| pr.gpu.emitters.get(&key));
                match group.and_then(|e| e.groups.as_ref()) {
                    Some((_, draw)) => {
                        pass.set_pipeline(pr.gpu_pipeline_for(batch.blend, reactive));
                        pass.set_bind_group(groups::DRAW, draw, &[]);
                        pass.draw_indexed(0..6, 0, batch.instances.clone());
                        pass.set_bind_group(groups::DRAW, &pr.globals_bind_group, &[]);
                    }
                    None if batch.gpu.is_some() => {}
                    None => {
                        pass.set_pipeline(pr.pipeline_for(batch.blend, reactive));
                        pass.draw_indexed(0..6, 0, batch.instances.clone());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "sprite_gpu_tests.rs"]
mod sprite_gpu_tests;
