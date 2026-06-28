//! src/dev/lighting_bake.rs — One-button lighting bake: auto-place + bake both sets.
//!
//! The orchestration behind `Lighting.Bake()` (the script verb) and the editor's
//! "Bake Lighting" button (#246). In ONE action it:
//!   1. auto-places light probes (a grid through the navigable volume / static AABB)
//!      and reflection probes (one per coarse region of the static AABB) — UNLESS that
//!      set is already manually authored, in which case its placement is left untouched;
//!   2. runs the two existing dev bakes — the full-bounce light-probe bake (#250,
//!      `probe_bake`) and the GGX reflection-probe bake (#252, `reflection_bake`).
//!
//! Placement is the deterministic, pure step (`scene::lighting_placement`); the bakes
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
//! Allowed deps: render (via the two bakes), scene (placement + data), navigation (read).

use crate::navigation::NavigationGraph;
use crate::scene::lighting::placement;
use crate::scene::Scene;

/// Tunables for the one-button bake. Defaults give a sensible mid-size-level layout;
/// the script verb exposes them as optional arguments.
#[derive(Clone, Copy, Debug)]
pub struct LightingBakeParams {
    /// Light-probe grid cell size, world units.
    pub probe_spacing: f32,
    /// Approximate reflection-probe room edge, world units (one probe per region).
    pub reflection_region: f32,
    /// Per-axis cap on reflection regions, bounding the bake cost on huge levels.
    pub reflection_cap: u32,
}

impl Default for LightingBakeParams {
    fn default() -> Self {
        Self {
            probe_spacing: 4.0,
            reflection_region: 12.0,
            reflection_cap: 4,
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
pub fn bake_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
) -> Result<LightingBakeReport, String> {
    let auto_placed_light = auto_place_light(scene, nav, params.probe_spacing);
    let auto_placed_reflection = auto_place_reflection(scene, &params);

    let light_probes = scene.probes.probes.len();
    let reflection_probes = scene.reflection_probes.len();

    let ran_light_bake = crate::dev::probe_bake::bake_scene_probes(scene)?;
    let ran_reflection_bake =
        crate::dev::reflection_bake::bake_scene_reflections(scene, scene_path)?;

    Ok(LightingBakeReport {
        light_probes,
        reflection_probes,
        auto_placed_light,
        auto_placed_reflection,
        ran_light_bake,
        ran_reflection_bake,
    })
}

/// Auto-place the light-probe grid when the set is empty; returns whether it placed.
/// A non-empty set is left as authored (manual-vs-auto rule).
fn auto_place_light(scene: &mut Scene, nav: Option<&NavigationGraph>, spacing: f32) -> bool {
    if !scene.probes.is_empty() {
        return false;
    }
    match placement::plan_light_probes(scene, nav, spacing) {
        Some(plan) => {
            scene.probes.fill_grid(plan.min, plan.max, plan.spacing);
            true
        }
        None => false,
    }
}

/// Auto-place reflection probes when the set is empty; returns whether it placed. A
/// non-empty set is left as authored (manual-vs-auto rule).
fn auto_place_reflection(scene: &mut Scene, params: &LightingBakeParams) -> bool {
    if !scene.reflection_probes.is_empty() {
        return false;
    }
    let plans =
        placement::plan_reflection_probes(scene, params.reflection_region, params.reflection_cap);
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
#[path = "lighting_bake_tests.rs"]
mod lighting_bake_tests;
