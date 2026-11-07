//! src/render/stats/frame.rs — filling a frame's [`RenderCounters`]: started per
//! render, each camera's solids and particles added, the frame's totals at its end.

use super::RenderCounters;
use crate::render::draw::resources::SolidResources;
use crate::render::gpu::textures::compressed::gpu_bytes;
use crate::render::gpu::uniforms::MAX_DIRECTIONAL_LIGHTS;
use crate::render::passes::particles::ParticleDraws;
use crate::render::{RenderView, Renderer};
use crate::scene::{LightType, Scene};

impl Renderer {
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
        let batches = solids.draws.batches();
        c.add_draws(batches.map(|b| (b.key.indices.len(), b.instances.len() as u32)));
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
        let c = &mut self.frame_counters;
        c.shadow_draws = self.shadow_renderer.drawn.len() as u32;
        let culls = self.shadow_renderer.culls;
        c.shadow_cull_tested = culls.tested;
        c.shadow_frustum_culled = culls.frustum;
        c.shadow_size_culled = culls.size;
        c.shadow_occlusion_culled = culls.occlusion;
        c.add_draws(self.shadow_renderer.drawn.iter().copied());
        c.ui_draws = view.ui.last_batches() as u32;
        c.ui_mask_passes = view.ui.last_mask_passes();
        c.ui_blur_passes = view.ui.last_blur_passes();
        c.draw_calls += c.ui_draws;
        let textures = self
            .gpu_textures
            .values()
            .chain(self.texture_fallbacks.values());
        c.texture_bytes = textures.map(|t| gpu_bytes(&t.texture)).sum();
        c.lightmap_bytes = self.lightmaps.gpu_bytes();
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
