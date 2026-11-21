//! crates/render/src/stats/frame.rs — filling a frame's [`RenderCounters`]: started per
//! render, each camera's solids and particles added, the frame's totals at its end.

use super::RenderCounters;
use crate::draw::batch::DrawBatch;
use crate::draw::resources::SolidResources;
use crate::gpu::textures::compressed::gpu_bytes;
use crate::gpu::uniforms::MAX_DIRECTIONAL_LIGHTS;
use crate::passes::particles::ParticleDraws;
use crate::{RenderView, Renderer};
use rusty_scene::{LightType, Scene};

impl Renderer {
    /// Every texture upload the renderer holds: the cached uploads and their
    /// uncompressed fallbacks (`texture_fallbacks`, a block format a slot cannot
    /// read), the set `texture_bytes` sums.
    pub fn resident_textures(&self) -> impl Iterator<Item = &wgpu::Texture> {
        let fallbacks = self.texture_fallbacks.values();
        self.gpu_textures
            .values()
            .chain(fallbacks)
            .map(|t| &t.texture)
    }

    /// Start a render's clocks: the GPU timer's frame, and the GPU call counts
    /// (`gpu::ops`) from zero, so what was counted before the render (setup, a
    /// warm-up, the last frame's timer resolve) belongs to no frame.
    pub(crate) fn begin_frame_clocks(&mut self) {
        self.gpu_timer.begin_frame();
        crate::gpu::ops::take();
    }

    /// Start a frame's counters: the light counts, and an empty shadow-draw log.
    pub(crate) fn begin_counters(&mut self, scene: &Scene) {
        let (lights, lights_dropped) = count_lights(scene);
        self.frame_counters = RenderCounters {
            lights,
            lights_dropped,
            ..Default::default()
        };
        self.shadow_renderer.drawn.clear();
        self.shadow_renderer.culls = Default::default();
    }

    /// Count one camera's solids (mesh particles among them) and sprite particle
    /// batches.
    pub(crate) fn count_camera(&mut self, solids: &SolidResources, particles: ParticleDraws) {
        let c = &mut self.frame_counters;
        let clustered = solids.culled.as_ref().and_then(|c| c.clusters.as_ref());
        let clustered = clustered.map_or(&[][..], |c| &c.draws[..]);
        c.add_draws(draws_of(solids.draws.batches(), clustered));
        c.visible_entities += solids.visible;
        c.cull_tested += solids.culls.tested;
        c.culled_entities += solids.culls.culled;
        c.size_culled_entities += solids.culls.size_culled;
        c.lod_hidden_entities += solids.culls.lod_hidden;
        c.occlusion_culled += solids.culls.occluded;
        c.draw_calls += particles.draw_calls;
        c.particles_drawn += particles.instances + solids.mesh_particles;
    }

    /// Close the frame's counters: the shadow casters drawn and the UI batches and
    /// effect passes.
    pub(crate) fn finish_counters(&mut self, view: &RenderView) {
        let texture_bytes = self.resident_textures().map(gpu_bytes).sum();
        let c = &mut self.frame_counters;
        c.shadow_draws = self.shadow_renderer.drawn.len() as u32;
        let culls = self.shadow_renderer.culls;
        c.shadow_cull_tested = culls.tested;
        c.shadow_frustum_culled = culls.frustum;
        c.shadow_size_culled = culls.size;
        c.shadow_occlusion_culled = culls.occlusion;
        c.shadow_gather_us = culls.gather_us;
        c.add_draws(self.shadow_renderer.drawn.iter().copied());
        c.ui_draws = view.ui.last_batches() as u32;
        c.ui_meshed = view.ui.last_meshed() as u32;
        c.ui_laid_out = view.ui.last_laid_out().into();
        c.ui_relaid = view.ui.last_relaid();
        c.ui_mask_passes = view.ui.last_mask_passes();
        c.ui_blur_passes = view.ui.last_blur_passes();
        c.draw_calls += c.ui_draws;
        c.texture_bytes = texture_bytes;
        c.lightmap_bytes = self.lightmaps.gpu_bytes();
        c.reflection_bytes = self.reflection_probes.gpu_bytes();
        c.mesh_count = self.gpu_meshes.len() as u32;
        c.mesh_bytes = self.gpu_meshes.values().map(mesh_bytes).sum();
        c.cluster_pool_bytes = self.hiz.clusters.pool.leased_bytes();
        c.uploads = std::mem::take(&mut self.uploads);
        c.uploads
            .add_all(std::mem::take(&mut self.lightmaps.uploads));
        c.ops = crate::gpu::ops::take();
    }
}

/// A GPU mesh's vertex and index buffers' bytes.
fn mesh_bytes(mesh: &crate::GpuMesh) -> u64 {
    mesh.vertex_buffer.size() + mesh.index_buffer.size()
}

impl RenderCounters {
    /// Add a set of indexed draws — `(num_indices, instances)` per draw call — to the
    /// counts. An instanced draw is one call but submits every instance's triangles.
    pub(crate) fn add_draws(&mut self, draws: impl IntoIterator<Item = (u32, u32)>) {
        for (indices, instances) in draws {
            self.draw_calls += 1;
            self.triangles += u64::from(indices / 3) * u64::from(instances);
        }
    }
}

/// `(active lights, lights with no uniform slot)` — mirrors `apply_scene_lights`,
/// which keeps four of the directional lights a frame draws live (`Baked` ones
/// never take a slot, #809) and the last ambient. Point and spot lights are
/// clustered (#434); the binning stage counts the ones a camera drops.
pub(crate) fn count_lights(scene: &Scene) -> (u32, u32) {
    let mut per_type = [0u32; 5];
    for id in scene.world.ids_with_light() {
        if !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let Some(light) = scene.world.light(id) else {
            continue;
        };
        per_type[match light.light_type {
            LightType::Point => 0,
            // A `Baked` sun takes no slot (#809): counted, never dropped.
            // Nor does an Area light, always `Baked` (#1674).
            LightType::Directional if !scene.light_mode(id, &light).renders_live(false) => 4,
            LightType::Area => 4,
            LightType::Directional => 1,
            LightType::Spotlight => 2,
            LightType::Ambient => 3,
        }] += 1;
    }
    let dropped =
        per_type[1].saturating_sub(MAX_DIRECTIONAL_LIGHTS as u32) + per_type[3].saturating_sub(1);
    (per_type.iter().sum(), dropped)
}

/// The draws `batches` make, `(indices, instances)` each: one instanced draw per
/// batch, but one draw per instance for the batch `i` that `clustered[i]` says is
/// drawn by cluster (#1968), whose kept clusters each instance draws on its own.
pub(super) fn draws_of<'a>(
    batches: impl Iterator<Item = &'a DrawBatch> + 'a,
    clustered: &'a [Option<std::ops::Range<u32>>],
) -> impl Iterator<Item = (u32, u32)> + 'a {
    batches.enumerate().flat_map(move |(i, b)| {
        let n = b.instances.len() as u32;
        let split = clustered.get(i).is_some_and(Option::is_some);
        let (calls, per) = if split { (n, 1) } else { (1, n) };
        std::iter::repeat_n((b.key.indices.len(), per), calls as usize)
    })
}

impl RenderCounters {
    /// Add one opaque scene pass's `batches` to the forward batch split (#2334):
    /// out of the prepass (`BatchKey::in_prepass`: the unlit ones), or in it in a
    /// pass `primed` or not.
    pub(crate) fn count_forward<'a>(
        &mut self,
        batches: impl Iterator<Item = &'a DrawBatch>,
        primed: bool,
    ) {
        for batch in batches {
            let count = match (batch.key.in_prepass(), primed) {
                (false, _) => &mut self.forward_unlit,
                (true, true) => &mut self.forward_lit_primed,
                (true, false) => &mut self.forward_lit_unprimed,
            };
            *count += 1;
        }
    }
}
