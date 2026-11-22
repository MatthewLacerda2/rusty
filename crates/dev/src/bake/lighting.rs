//! The probe bake behind `Lighting.Bake()` and Generate Lighting's probe step: re-place the
//! placed probes around the manual ones, then bake both sets.

use super::progress::{stage_progress, BakeLog};
use super::steps::LightingBake;
use glam::Vec3;

use rusty_lighting::lightmap::{BakeMesh, BakeScene};
use rusty_lighting::placement::{self, ReflectionPlacement, Site, Solid};
use rusty_navigation::NavigationGraph;
use rusty_physics::collider_world_triangles;
use rusty_scene::lighting::probe::ProbeVolume;
use rusty_scene::{ReflectionProbeSet, Scene};

#[derive(Clone, Copy, Debug)]
pub struct LightingBakeParams {
    pub probe_spacing: f32,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightingBakeReport {
    pub light_probes: usize,
    pub reflection_probes: usize,
    pub auto_placed_light: bool,
    pub auto_placed_reflection: bool,
    pub ran_light_bake: bool,
    pub ran_reflection_bake: bool,
}

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

pub fn start_lighting(
    scene: &Scene,
    scene_path: Option<&str>,
    nav: Option<&NavigationGraph>,
    params: LightingBakeParams,
) -> Result<LightingBake, String> {
    let (planned, report) = plan_probes(scene, nav, params);
    LightingBake::start(scene, scene_path, planned, report)
}

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

pub struct PlannedProbes {
    pub light: ProbeVolume,
    pub reflections: ReflectionProbeSet,
    /// The scene index of each manual reflection probe kept: they lead `reflections`, in order.
    pub kept: Vec<usize>,
}

fn survey(scene: &Scene, nav: Option<&NavigationGraph>, floor_cell: f32) -> Option<Site> {
    let floor = nav.and_then(|nav| {
        let reach = nav.reachable_spans(&actor_starts(scene));
        nav.floor_grid(scene.nav_settings.agent_radius, Some(&reach))
    });
    Site::survey(scene, floor, Some(&static_solids(scene)), floor_cell)
}

pub(in crate::bake) fn actor_starts(scene: &Scene) -> Vec<Vec3> {
    let world = &scene.world;
    (world.ids_with_nav_agent().into_iter())
        .chain(world.ids_with_character_controller())
        .filter(|&id| world.is_active_in_hierarchy(id))
        .filter_map(|id| world.world_pose(id).map(|p| p.pos))
        .collect()
}

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
    // Then the meshes with no solid collider: a ceiling that is a render mesh alone roofs too.
    let meshes = (BakeScene::gather(scene, &|_, _| None).meshes.into_iter())
        .filter(|m| !solid_collider(m.entity))
        .map(|m| Solid {
            triangles: opaque_triangles(&m),
            vertices: m.positions,
            convex: false,
        });
    colliders.chain(meshes).collect()
}

fn opaque_triangles(mesh: &BakeMesh) -> Vec<[u32; 3]> {
    (mesh.indices.chunks_exact(3).enumerate())
        .filter(|(t, _)| mesh.run_at(*t as u32 * 3).occludes)
        .map(|(_, t)| [t[0], t[1], t[2]])
        .collect()
}

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
