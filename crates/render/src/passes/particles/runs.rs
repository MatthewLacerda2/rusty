//! crates/render/src/passes/particles/runs.rs — what each visible sprite emitter
//! draws for one camera, farthest emitter first: a `Cpu` emitter's instances, or a
//! GPU-simulated one's buffer (#2170).

use super::gpu::GpuParticles;
use super::instance::{emitter_instances, emitter_light, Eye, ParticleInstance};
use crate::draw::batch::sort::{back_to_front, view_depth};
use rusty_asset::MapRef;
use rusty_components::particle::ParticleBlend;
use rusty_components::ParticleRenderMode;
use rusty_scene::{Scene, SceneId};

/// What one emitter draws from.
pub(super) enum Sprites {
    /// Instances built this frame (a `Cpu` emitter).
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
            let slots = emitter.runtime.gpu.extent().min(buffers.capacity);
            if slots == 0 {
                // An idle emitter draws nothing, and so costs no draw and splits
                // no batch, as an empty `Cpu` one.
                continue;
            }
            let run = EmitterRun {
                blend: emitter.blend,
                texture: emitter.texture.clone(),
                sprites: Sprites::Gpu((scene.id(), id), slots, emitter.live_count() as u32),
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
