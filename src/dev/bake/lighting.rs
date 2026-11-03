//! src/dev/bake/lighting.rs — One-button lighting bake: auto-place + bake both sets.
//!
//! The orchestration behind `Lighting.Bake()` (the script verb) and the probe step
//! of the editor's Generate Lighting button (#246, #832). In ONE action it:
//!   1. auto-places light probes (a lattice through the air over the reachable floor,
//!      denser near geometry, #1737) and reflection probes (one per room of the nav
//!      surface's reachable floor plan, else of the static colliders', #1584),
//!      keeping every probe marked manual and planning around it;
//!   2. runs the two existing dev bakes — the multi-bounce light-probe bake (#250, #285,
//!      `probe_bake`) and the GGX reflection-probe bake (#252, `reflection_bake`) — as
//!      one job stepped a probe or a few at a time (`steps`, #1602).
//!
//! Placement is the deterministic, pure step (`scene::lighting::placement`); the bakes
//! are the render/dev step. This module is the single path both callers route through,
//! so the button and the API never drift (editor↔API parity). It is dev-only — a baked
//! lighting set is an authoring artifact (SH sidecar + KTX2 cubemaps) the runtime loads.
//!
//! Manual-vs-auto rule (#1737): a probe is either **manual** (`Probe.Add`,
//! `Probe.FillGrid`, `Reflection.Add`, the Inspector's Add, or marked so) or
//! **placed**. Every bake re-places the placed ones and never touches a manual one:
//! its position, box and settings stay, a placed reflection probe leaves the room
//! a manual one covers to it, and a placed light probe keeps clear of a manual one.
//! A saved probe without the field reads as placed. `Probe.Clear` /
//! `Reflection.Clear` still remove both kinds.
//!
//! It reads `navigation` only for the reachable floor plan (and, with nothing solid,
//! the walkable extent), and `physics` only for the static colliders' triangles,
//! both handed to placement as plain data.

use super::progress::{stage_progress, BakeLog};
use super::steps::LightingBake;
use glam::Vec3;

use crate::navigation::NavigationGraph;
use crate::physics::collider_world_triangles;
use crate::scene::lighting::placement::{self, ReflectionPlacement, Site, Solid};
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
/// Runs [`start_lighting`]'s job to the end, its progress lines to `say` and `log`,
/// and commits it. Refused while the editor's Generate Lighting runs.
pub fn bake_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
    say: &mut dyn FnMut(String),
    log: &mut BakeLog,
) -> Result<LightingBakeReport, String> {
    super::generate_lighting::refuse_while_baking(scene)?;
    let mut job = start_lighting(scene, scene_path, nav, params)?;
    run_to_end(&mut job, scene, say, log)?;
    job.commit(scene)?;
    log.finish();
    Ok(job.report())
}

/// Step `job` until it is done, its progress lines to `say` and where it is to `log`
/// after every step (#1667).
pub(super) fn run_to_end(
    job: &mut LightingBake,
    scene: &mut Scene,
    say: &mut dyn FnMut(String),
    log: &mut BakeLog,
) -> Result<(), String> {
    loop {
        if let Some(progress) = stage_progress(job.stage()) {
            log.observe(progress);
        }
        if job.step(scene, say)? {
            return Ok(());
        }
    }
}

/// Auto-place (per the manual-vs-auto rule) and return the bake of both probe sets
/// as a job to step ([`LightingBake::step`]): the editor takes one step a frame.
/// Errors when reflection probes have no saved scene to write beside.
pub fn start_lighting(
    scene: &mut Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
) -> Result<LightingBake, String> {
    let site = survey(scene, nav, params.reflection.cell);
    let report = LightingBakeReport {
        auto_placed_light: auto_place_light(scene, site.as_ref(), nav, params.probe_spacing),
        auto_placed_reflection: auto_place_reflection(scene, site.as_ref(), &params.reflection),
        light_probes: scene.probes.probes.len(),
        reflection_probes: scene.reflection_probes.len(),
        ..Default::default()
    };
    LightingBake::start(scene, scene_path, report)
}

/// The site both probe sets are planned on (#1737): the nav surface's reachable
/// floor (the largest walkable area and wherever an actor starts) when one is
/// baked, and the static colliders' triangles. `None` with nothing to place on.
fn survey(scene: &Scene, nav: Option<&NavigationGraph>, floor_cell: f32) -> Option<Site> {
    let floor = nav.and_then(|nav| {
        let reach = nav.reachable_spans(&actor_starts(scene));
        nav.floor_grid(scene.nav_settings.agent_radius, Some(&reach))
    });
    Site::survey(scene, floor, Some(&static_solids(scene)), floor_cell)
}

/// Where actors start: every active agent and character controller.
fn actor_starts(scene: &Scene) -> Vec<Vec3> {
    let world = &scene.world;
    (world.ids_with_nav_agent().into_iter())
        .chain(world.ids_with_character_controller())
        .filter(|&id| world.is_active_in_hierarchy(id))
        .filter_map(|id| world.world_pose(id).map(|p| p.pos))
        .collect()
}

/// The static, active, solid colliders as world triangles, in entity order.
fn static_solids(scene: &Scene) -> Vec<Solid> {
    let world = &scene.world;
    (world.ids_with_collider().into_iter())
        .filter(|&id| world.is_static(id) && world.is_active_in_hierarchy(id))
        .filter(|&id| {
            world
                .collider(id)
                .is_some_and(|c| c.active && !c.is_trigger)
        })
        .filter_map(|id| collider_world_triangles(scene, id))
        .map(|t| Solid {
            vertices: t.vertices,
            triangles: t.triangles,
            convex: t.convex,
        })
        .collect()
}

/// Re-place the light probes the automatic placement made, keeping the manual
/// ones (#1737); returns whether it placed any. From the site's occupancy grid,
/// else (nothing solid to grid) a regular grid over the walkable extent when the
/// set holds no manual probe.
fn auto_place_light(
    scene: &mut Scene,
    site: Option<&Site>,
    nav: Option<&NavigationGraph>,
    spacing: f32,
) -> bool {
    let manual: Vec<Vec3> = (scene.probes.probes.iter())
        .filter(|p| p.manual)
        .map(|p| p.position)
        .collect();
    let placed = match site {
        Some(site) => site.plan_light_probes(spacing, &manual),
        None if manual.is_empty() => {
            let walkable = nav.and_then(NavigationGraph::walkable_aabb);
            if let Some(plan) = placement::plan_light_probes(scene, walkable, spacing) {
                scene.probes.fill_grid(plan.min, plan.max, plan.spacing);
                return true;
            }
            Vec::new()
        }
        None => Vec::new(),
    };
    let any = !placed.is_empty();
    scene.probes.replace_placed(placed);
    any
}

/// Re-place the reflection probes the automatic placement made, keeping the
/// manual ones and the rooms they cover (#1737); returns whether it placed any.
fn auto_place_reflection(
    scene: &mut Scene,
    site: Option<&Site>,
    params: &ReflectionPlacement,
) -> bool {
    scene.reflection_probes.probes.retain(|p| p.manual);
    let Some(site) = site else {
        return false;
    };
    let params = ReflectionPlacement {
        max_probes: scene.lighting_settings.reflections.max_probes as usize,
        ..*params
    };
    let manual = scene.reflection_probes.probes.clone();
    let plans = placement::plan_reflection_probes(scene, site, &params, &manual);
    for plan in &plans {
        let index = scene
            .reflection_probes
            .add(plan.position, plan.box_max - plan.position);
        scene.reflection_probes.edit(index, |p| {
            (p.box_min, p.box_max) = (plan.box_min, plan.box_max);
            p.box_projection = plan.box_projection;
            p.importance = plan.importance;
        });
    }
    !plans.is_empty()
}

#[cfg(test)]
#[path = "lighting_tests.rs"]
mod lighting_tests;
