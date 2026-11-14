//! crates/dev/src/bake/lighting.rs — One-button lighting bake: auto-place + bake both sets.
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
//! the walkable extent), `physics` only for the static colliders' triangles, and
//! the lightmap gather for the static meshes' opaque triangles, all handed to
//! placement as plain data.

use super::progress::{stage_progress, BakeLog};
use super::steps::LightingBake;
use glam::Vec3;

use rusty_lighting::lightmap::{BakeMesh, BakeScene};
use rusty_lighting::placement::{self, ReflectionPlacement, Site, Solid};
use rusty_navigation::NavigationGraph;
use rusty_physics::collider_world_triangles;
use rusty_scene::lighting::probe::ProbeVolume;
use rusty_scene::{ReflectionProbeSet, Scene};

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
/// need a saved scene path, whose `<scene>.reflections/` folder takes the cubemaps,
/// and a stale cube there must be removable). A missing GPU adapter is NOT
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
    log.enter(crate::bake::progress::Step::ProbePlacement);
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

/// Plan the probe sets (per the manual-vs-auto rule) and return the bake of them as
/// a job to step ([`LightingBake::step`]): the editor takes one step a frame. The
/// scene keeps its probes and their lighting until the job commits, as Unity's bake
/// does: a bake that is refused, dropped or fails leaves the old lighting (#1737).
/// Errors when reflection probes have no saved scene whose `<scene>.reflections/`
/// folder takes their cubemaps.
pub fn start_lighting(
    scene: &Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
) -> Result<LightingBake, String> {
    let (planned, report) = plan_probes(scene, nav, params);
    LightingBake::start(scene, scene_path, planned, report)
}

/// Plan both probe sets of `scene` (per the manual-vs-auto rule) without touching
/// it: the sets a bake applies at its commit, and the placement's report.
pub(super) fn plan_probes(
    scene: &Scene,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
) -> (PlannedProbes, LightingBakeReport) {
    let site = survey(scene, nav, params.reflection.cell);
    let mut planned = PlannedProbes {
        light: scene.probes.clone(),
        reflections: scene.reflection_probes.clone(),
        kept: Vec::new(),
    };
    let spacing = params.probe_spacing;
    let site = site.as_ref();
    let auto_placed_light = auto_place_light(&mut planned.light, scene, site, nav, spacing);
    let auto_placed_reflection =
        auto_place_reflection(&mut planned, scene, site, &params.reflection);
    let report = LightingBakeReport {
        auto_placed_light,
        auto_placed_reflection,
        light_probes: planned.light.probes().len(),
        reflection_probes: planned.reflections.len(),
        ..Default::default()
    };
    (planned, report)
}

/// The probe sets a bake plans at its start and applies at its commit (#1737).
pub struct PlannedProbes {
    pub light: ProbeVolume,
    pub reflections: ReflectionProbeSet,
    /// The scene index of each manual reflection probe kept, in order: they lead
    /// `reflections`, and the commit takes them from the scene as it is then, so an
    /// edit the bake does not read (an intensity) survives it.
    pub kept: Vec<usize>,
}

/// The site both probe sets are planned on (#1737): the nav surface's reachable
/// floor (the main floor, always, and wherever an actor starts) when one is
/// baked, and the static colliders' triangles. `None` with nothing to place on.
fn survey(scene: &Scene, nav: Option<&NavigationGraph>, floor_cell: f32) -> Option<Site> {
    let floor = nav.and_then(|nav| {
        let reach = nav.reachable_spans(&actor_starts(scene));
        nav.floor_grid(scene.nav_settings.agent_radius, Some(&reach))
    });
    Site::survey(scene, floor, Some(&static_solids(scene)), floor_cell)
}

/// Where actors start: every active agent and character controller.
pub(in crate::bake) fn actor_starts(scene: &Scene) -> Vec<Vec3> {
    let world = &scene.world;
    (world.ids_with_nav_agent().into_iter())
        .chain(world.ids_with_character_controller())
        .filter(|&id| world.is_active_in_hierarchy(id))
        .filter_map(|id| world.world_pose(id).map(|p| p.pos))
        .collect()
}

/// What is solid in the static scene as world triangles: the active, solid
/// colliders, then the opaque runs of the static meshes whose entity has none (a
/// ceiling built as a render mesh alone still roofs its room). Transparent runs
/// are left out, so a liquid or a pane of glass is not solid. In entity order.
fn static_solids(scene: &Scene) -> Vec<Solid> {
    let world = &scene.world;
    let solid_collider = |id: u32| {
        world.is_static(id)
            && world.is_active_in_hierarchy(id)
            && world
                .collider(id)
                .is_some_and(|c| c.active && !c.is_trigger)
    };
    let colliders = (world.ids_with_collider().into_iter())
        .filter(|&id| solid_collider(id))
        .filter_map(|id| collider_world_triangles(scene, id))
        .map(|t| Solid {
            vertices: t.vertices,
            triangles: t.triangles,
            convex: t.convex,
        });
    let meshes = (BakeScene::gather(scene, &|_| None).meshes.into_iter())
        .filter(|m| !solid_collider(m.entity))
        .map(|m| Solid {
            triangles: opaque_triangles(&m),
            vertices: m.positions,
            convex: false,
        });
    colliders.chain(meshes).collect()
}

/// The triangles of `mesh`'s opaque runs.
fn opaque_triangles(mesh: &BakeMesh) -> Vec<[u32; 3]> {
    (mesh.indices.chunks_exact(3).enumerate())
        .filter(|(t, _)| mesh.run_at(*t as u32 * 3).occludes)
        .map(|(_, t)| [t[0], t[1], t[2]])
        .collect()
}

/// Re-place `volume`'s light probes the automatic placement made, keeping the
/// manual ones (#1737); returns whether it placed any. From the site's occupancy
/// grid, else (nothing solid to grid) a regular grid over the walkable extent when
/// the set holds no manual probe.
fn auto_place_light(
    volume: &mut ProbeVolume,
    scene: &Scene,
    site: Option<&Site>,
    nav: Option<&NavigationGraph>,
    spacing: f32,
) -> bool {
    let manual: Vec<Vec3> = (volume.probes().iter())
        .filter(|p| p.manual)
        .map(|p| p.position)
        .collect();
    let placed = match site {
        Some(site) => site.plan_light_probes(spacing, &manual),
        None if manual.is_empty() => {
            let walkable = nav.and_then(NavigationGraph::walkable_aabb);
            if let Some(plan) = placement::plan_light_probes(scene, walkable, spacing) {
                volume.fill_grid(plan.min, plan.max, plan.spacing);
                return true;
            }
            Vec::new()
        }
        None => Vec::new(),
    };
    let any = !placed.is_empty();
    volume.replace_placed(placed);
    any
}

/// Re-place the planned reflection probes the automatic placement made, keeping
/// the manual ones and the rooms they cover (#1737); returns whether it placed any.
fn auto_place_reflection(
    planned: &mut PlannedProbes,
    scene: &Scene,
    site: Option<&Site>,
    params: &ReflectionPlacement,
) -> bool {
    let set = &mut planned.reflections;
    planned.kept = (set.probes.iter().enumerate())
        .filter(|(_, p)| p.manual)
        .map(|(i, _)| i)
        .collect();
    set.probes.retain(|p| p.manual);
    let Some(site) = site else {
        return false;
    };
    let params = ReflectionPlacement {
        max_probes: scene.lighting_settings.reflections.max_probes as usize,
        ..*params
    };
    let manual = set.probes.clone();
    let plans = placement::plan_reflection_probes(scene, site, &params, &manual);
    for plan in &plans {
        let index = set.add(plan.position, plan.box_max - plan.position);
        set.edit(index, |p| {
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
#[cfg(test)]
#[path = "solids_tests.rs"]
mod solids_tests;
