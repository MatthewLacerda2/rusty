//! Probe placement on a baked nav surface (#1737): `scene::lighting::placement`
//! surveys the reachable floor `NavigationGraph::floor_grid` plans, as
//! `dev::bake::lighting` asks it to. The placement's own tests are `scene`'s; these
//! are the half that bakes a navmesh.

use glam::Vec3;

use super::NavigationGraph;
use rusty_lighting::placement::{plan_reflection_probes, ReflectionPlacement, ReflectionPlan, Site};
use rusty_scene::Scene;

mod light_tests;
mod liquid_tests;
mod reflection_tests;

/// `scene`'s reflection plans on its baked nav surface's reachable floor.
pub(super) fn plan_on_nav(scene: &Scene) -> Vec<ReflectionPlan> {
    plan_on_nav_from(scene, &[])
}

/// [`plan_on_nav`] with actors starting at `starts`.
pub(super) fn plan_on_nav_from(scene: &Scene, starts: &[Vec3]) -> Vec<ReflectionPlan> {
    let site = site_on_nav_from(scene, starts);
    plan_reflection_probes(scene, &site, &ReflectionPlacement::default(), &[])
}

/// `scene`'s site on its baked nav surface's reachable floor.
pub(super) fn site_on_nav(scene: &Scene) -> Site {
    site_on_nav_from(scene, &[])
}

/// [`site_on_nav`] with actors starting at `starts`.
pub(super) fn site_on_nav_from(scene: &Scene, starts: &[Vec3]) -> Site {
    let nav = NavigationGraph::from_scene(scene);
    let reach = nav.reachable_spans(starts);
    let floor = nav.floor_grid(scene.nav_settings.agent_radius, Some(&reach));
    Site::survey(scene, floor, None, 0.5).expect("a site")
}
