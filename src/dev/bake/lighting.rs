//! src/dev/bake/lighting.rs — One-button lighting bake: auto-place + bake both sets.
//!
//! The orchestration behind `Lighting.Bake()` (the script verb) and the probe step
//! of the editor's Generate Lighting button (#246, #832). In ONE action it:
//!   1. auto-places light probes (a grid through the navigable volume / static AABB)
//!      and reflection probes (one per room of the nav surface's floor plan, else of
//!      the static colliders', #1584) — UNLESS that
//!      set is already manually authored, in which case its placement is left untouched;
//!   2. runs the two existing dev bakes — the multi-bounce light-probe bake (#250, #285,
//!      `probe_bake`) and the GGX reflection-probe bake (#252, `reflection_bake`) — as
//!      one job stepped a probe or a few at a time (`steps`, #1602).
//!
//! Placement is the deterministic, pure step (`scene::lighting::placement`); the bakes
//! are the render/dev step. This module is the single path both callers route through,
//! so the button and the API never drift (editor↔API parity). It is dev-only — a baked
//! lighting set is an authoring artifact (SH sidecar + KTX2 cubemaps) the runtime loads.
//!
//! Manual-vs-auto rule (documented): placement is decided PER SET. If the scene already
//! carries any light probes, their layout is kept and only rebaked; same independently
//! for reflection probes. Auto-placement runs only for a set that is currently empty.
//! So a level with no probes gets a full auto-place + bake; one with hand-authored
//! probes is a pure rebake — manual placement is never clobbered.
//!
//! It reads `navigation` only for the walkable extent light probes fill and the
//! floor plan reflection probes are placed on, handed to placement as plain data.

use super::steps::LightingBake;
use crate::navigation::NavigationGraph;
use crate::scene::lighting::placement::{self, ReflectionPlacement};
use crate::scene::Scene;

/// Tunables for the one-button bake. Defaults give a sensible mid-size-level layout;
/// the script verb exposes them as optional arguments.
#[derive(Clone, Copy, Debug)]
pub struct LightingBakeParams {
    /// Light-probe grid cell size, world units.
    pub probe_spacing: f32,
    /// How reflection probes are placed: one per room, none wider than its region.
    pub reflection: ReflectionPlacement,
}

impl Default for LightingBakeParams {
    fn default() -> Self {
        Self {
            probe_spacing: 4.0,
            reflection: ReflectionPlacement::default(),
        }
    }
}

/// What the bake did, for the caller to log. Counts are post-placement; `ran_*`
/// reflect whether each GPU bake actually executed (vs skipped for no adapter).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightingBakeReport {
    pub light_probes: usize,
    pub reflection_probes: usize,
    pub auto_placed_light: bool,
    pub auto_placed_reflection: bool,
    pub ran_light_bake: bool,
    pub ran_reflection_bake: bool,
}

/// Auto-place (per the manual-vs-auto rule) then bake both probe sets. Returns a
/// report on success; errors only when a bake's own contract fails (e.g. reflections
/// need a saved scene path to write the cubemaps beside). A missing GPU adapter is NOT
/// an error — each bake skips gracefully and the report's `ran_*` flag is `false`.
/// Runs [`start_lighting`]'s job to the end, its progress lines to `say`.
pub fn bake_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
    say: &mut dyn FnMut(String),
) -> Result<LightingBakeReport, String> {
    let mut job = start_lighting(scene, scene_path, nav, params);
    while !job.step(scene, say)? {}
    Ok(job.report())
}

/// Auto-place (per the manual-vs-auto rule) and return the bake of both probe sets
/// as a job to step ([`LightingBake::step`]): the editor takes one step a frame.
pub fn start_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
) -> LightingBake {
    let report = LightingBakeReport {
        auto_placed_light: auto_place_light(scene, nav, params.probe_spacing),
        auto_placed_reflection: auto_place_reflection(scene, nav, &params.reflection),
        light_probes: scene.probes.probes.len(),
        reflection_probes: scene.reflection_probes.len(),
        ..Default::default()
    };
    LightingBake::start(scene, scene_path, report)
}

/// Auto-place the light-probe grid when the set is empty; returns whether it placed.
/// A non-empty set is left as authored (manual-vs-auto rule).
fn auto_place_light(scene: &mut Scene, nav: Option<&NavigationGraph>, spacing: f32) -> bool {
    if !scene.probes.is_empty() {
        return false;
    }
    let walkable = nav.and_then(NavigationGraph::walkable_aabb);
    match placement::plan_light_probes(scene, walkable, spacing) {
        Some(plan) => {
            scene.probes.fill_grid(plan.min, plan.max, plan.spacing);
            true
        }
        None => false,
    }
}

/// Auto-place reflection probes when the set is empty; returns whether it placed. A
/// non-empty set is left as authored (manual-vs-auto rule).
fn auto_place_reflection(
    scene: &mut Scene,
    nav: Option<&NavigationGraph>,
    params: &ReflectionPlacement,
) -> bool {
    if !scene.reflection_probes.is_empty() {
        return false;
    }
    let radius = scene.nav_settings.agent_radius;
    let floor = nav.and_then(|nav| nav.floor_grid(radius));
    let params = ReflectionPlacement {
        max_probes: scene.lighting_settings.reflections.max_probes as usize,
        ..*params
    };
    let plans = placement::plan_reflection_probes(scene, floor, &params);
    if plans.is_empty() {
        return false;
    }
    for plan in plans {
        let index = scene
            .reflection_probes
            .add(plan.position, plan.box_max - plan.position);
        scene
            .reflection_probes
            .set_box(index, plan.box_min, plan.box_max);
    }
    true
}

#[cfg(test)]
#[path = "lighting_tests.rs"]
mod lighting_tests;
