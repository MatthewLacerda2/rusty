//! src/render/stats.rs — what one rendered frame submitted (#433).
//!
//! `Renderer::render` fills a [`RenderCounters`] per frame so an agent can measure a
//! rendering change (instancing, LODs, clustered lights) instead of guessing. The
//! counts are taken from the draw lists the frame actually built — cheap, and exact
//! for everything that issues geometry. The dev layer folds them into `FrameStats`.

use crate::render::draw::resources::SolidResources;
use crate::render::{RenderView, Renderer};
use crate::scene::{LightType, Scene};

/// The forward lighting uniform's slot count per light type: 4 point lights, and one
/// each of directional, spot and ambient (a later one overwrites an earlier one).
const POINT_LIGHT_SLOTS: u32 = 4;

/// Counters for one `Renderer::render` call, summed over every camera in the stack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderCounters {
    /// Geometry draw calls: solids, transparents, shadow casters, decals, particle
    /// batches and UI batches. Fullscreen post-FX and the skybox are not counted.
    pub draw_calls: u32,
    /// Triangles submitted by the solid, transparent and shadow draws.
    pub triangles: u64,
    /// Mesh entities drawn, summed over the camera stack.
    pub visible_entities: u32,
    /// Mesh entities the frustum cull skipped, summed over the camera stack.
    pub culled_entities: u32,
    /// Active lights in the scene.
    pub lights: u32,
    /// Active lights the forward uniform had no slot for — silently unlit today.
    pub lights_dropped: u32,
    /// Shadow-caster draws (static bake + dynamic).
    pub shadow_draws: u32,
    /// UI batches drawn.
    pub ui_draws: u32,
}

impl RenderCounters {
    /// Every counter under its `FrameStats` metric name.
    pub fn pairs(&self) -> [(&'static str, u64); 8] {
        [
            ("draw_calls", self.draw_calls.into()),
            ("triangles", self.triangles),
            ("visible_entities", self.visible_entities.into()),
            ("culled_entities", self.culled_entities.into()),
            ("lights", self.lights.into()),
            ("lights_dropped", self.lights_dropped.into()),
            ("shadow_draws", self.shadow_draws.into()),
            ("ui_draws", self.ui_draws.into()),
        ]
    }

    /// Add a set of indexed draws (`num_indices` per draw) to the counts.
    pub(crate) fn add_draws(&mut self, indices: impl IntoIterator<Item = u32>) {
        for n in indices {
            self.draw_calls += 1;
            self.triangles += u64::from(n / 3);
        }
    }
}

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
    }

    /// Count one camera's solids, decals and particle batches.
    pub(crate) fn count_camera(&mut self, solids: &SolidResources, decals: usize, particles: u32) {
        let c = &mut self.frame_counters;
        let transparent = solids.transparent.iter().map(|(r, _)| r);
        c.add_draws(solids.opaque.iter().chain(transparent).map(|r| r.2));
        c.visible_entities += (solids.opaque.len() + solids.transparent.len()) as u32;
        c.culled_entities += solids.culled;
        c.draw_calls += decals as u32 + particles;
    }

    /// Close the frame's counters: the shadow casters drawn and the UI batches.
    pub(crate) fn finish_counters(&mut self, view: &RenderView) {
        let c = &mut self.frame_counters;
        c.shadow_draws = self.shadow_renderer.drawn.len() as u32;
        c.add_draws(self.shadow_renderer.drawn.iter().copied());
        c.ui_draws = view.ui.last_batches() as u32;
        c.draw_calls += c.ui_draws;
    }
}

/// `(active lights, lights with no uniform slot)` — mirrors `apply_scene_lights`,
/// which keeps the first four point lights and the last of each other type.
pub(crate) fn count_lights(scene: &Scene) -> (u32, u32) {
    let mut per_type = [0u32; 4];
    for id in scene.world.ids_with_light() {
        if !scene.world.is_active(id) {
            continue;
        }
        let Some(light) = scene.world.light(id) else {
            continue;
        };
        per_type[match light.light_type {
            LightType::Point => 0,
            LightType::Directional => 1,
            LightType::Spotlight => 2,
            LightType::Ambient => 3,
        }] += 1;
    }
    let dropped = per_type[0].saturating_sub(POINT_LIGHT_SLOTS)
        + per_type[1..]
            .iter()
            .map(|n| n.saturating_sub(1))
            .sum::<u32>();
    (per_type.iter().sum(), dropped)
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
