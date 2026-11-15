//! crates/render/src/passes/particles/runs.rs — what each visible sprite emitter
//! draws for one camera, farthest emitter first: a `Cpu` emitter's instances, or a
//! GPU-simulated one's buffer (#2170), or that one's CPU fallback as instances.

use glam::Vec3;

use super::gpu::emitter::EmitterBuffers;
use super::gpu::record::{sprite, GpuParticle};
use super::gpu::GpuParticles;
use super::instance::{emitter_instances, emitter_light, Eye, ParticleInstance};
use crate::draw::batch::sort::{back_to_front, view_depth};
use rusty_asset::MapRef;
use rusty_components::particle::ParticleBlend;
use rusty_components::ParticleRenderMode;
use rusty_scene::{Scene, SceneId};

/// What one emitter draws from.
pub(super) enum Sprites {
    /// Instances built this frame (a `Cpu` emitter, or a `Gpu` one's CPU fallback).
    Instances(Vec<ParticleInstance>),
    /// A GPU-simulated emitter's buffer (#2170): its key and slots in use, and the
    /// live particles it holds (for the counters).
    Gpu((SceneId, u32), u32, u32),
}

/// One emitter's sprites, before textures are resolved.
pub(super) struct EmitterRun {
    pub(super) blend: ParticleBlend,
    pub(super) texture: Option<MapRef>,
    pub(super) sprites: Sprites,
}

/// Every active, visible sprite emitter's instances, farthest emitter first; a
/// GPU-simulated one's buffer from `gpu`.
pub(super) fn collect_runs(
    scene: &Scene,
    culling_mask: u32,
    eye: Eye,
    gpu: &GpuParticles,
) -> Vec<EmitterRun> {
    let mut runs: Vec<(EmitterRun, f32)> = Vec::new();
    for id in scene.world.ids_with_particles() {
        // Inactive emitters, and those on a layer the camera culls (#92), don't draw.
        if !scene.world.is_active_in_hierarchy(id)
            || !rusty_scene::layer_in_mask(scene.world.layer(id), culling_mask)
        {
            continue;
        }
        let emitter = scene
            .world
            .particles(id)
            .expect("id came from ids_with_particles");
        let origin = scene.world.world_position(id);
        if emitter.on_gpu() {
            // A `Gpu` emitter draws mesh particles as billboards (#2170).
            let Some(buffers) = gpu.emitters.get(&(scene.id(), id)) else {
                continue;
            };
            let sprites = match &buffers.mirror {
                Some(mirror) => {
                    Sprites::Instances(mirror_instances(mirror, buffers, eye, emitter.blend))
                }
                None => {
                    let slots = emitter.runtime.gpu.extent().min(buffers.capacity);
                    Sprites::Gpu((scene.id(), id), slots, emitter.live_count() as u32)
                }
            };
            let run = EmitterRun {
                blend: emitter.blend,
                texture: emitter.texture.clone(),
                sprites,
            };
            runs.push((run, view_depth(origin, eye.pos, eye.fwd)));
            continue;
        }
        if emitter.runtime.particles.is_empty() || emitter.render.mode == ParticleRenderMode::Mesh {
            continue;
        }
        let light = emitter_light(scene, &emitter, origin);
        let (instances, depth) = emitter_instances(&emitter, light, eye);
        let run = EmitterRun {
            blend: emitter.blend,
            texture: emitter.texture.clone(),
            sprites: Sprites::Instances(instances),
        };
        runs.push((run, depth));
    }
    back_to_front(&mut runs);
    runs.into_iter().map(|(run, _)| run).collect()
}

/// A GPU-simulated emitter's CPU fallback as instances, alpha ones back to front.
fn mirror_instances(
    mirror: &[GpuParticle],
    buffers: &EmitterBuffers,
    eye: Eye,
    blend: ParticleBlend,
) -> Vec<ParticleInstance> {
    let mut keyed: Vec<(ParticleInstance, f32)> = mirror
        .iter()
        .take(buffers.emitter.info[1] as usize)
        .filter_map(|p| sprite(p, &buffers.emitter, buffers.baked()))
        .map(|i| {
            let depth = view_depth(Vec3::from(i.center), eye.pos, eye.fwd);
            (i, depth)
        })
        .collect();
    if blend == ParticleBlend::Alpha {
        back_to_front(&mut keyed);
    }
    keyed.into_iter().map(|(i, _)| i).collect()
}
