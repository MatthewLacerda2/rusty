//! The half of probe placement that bakes a navmesh; the placement's own tests are `lighting`'s.

use glam::Vec3;

use super::NavigationGraph;
use rusty_lighting::placement::{
    plan_reflection_probes, ReflectionPlacement, ReflectionPlan, Site,
};
use rusty_scene::Scene;

mod light_tests;
mod liquid_tests;
mod reflection_tests;

pub(super) fn plan_on_nav(scene: &Scene) -> Vec<ReflectionPlan> {
    plan_on_nav_from(scene, &[])
}

pub(super) fn plan_on_nav_from(scene: &Scene, starts: &[Vec3]) -> Vec<ReflectionPlan> {
    let site = site_on_nav_from(scene, starts);
    plan_reflection_probes(scene, &site, &ReflectionPlacement::default(), &[])
}

pub(super) fn site_on_nav(scene: &Scene) -> Site {
    site_on_nav_from(scene, &[])
}

pub(super) fn site_on_nav_from(scene: &Scene, starts: &[Vec3]) -> Site {
    let nav = NavigationGraph::from_scene(scene);
    let reach = nav.reachable_spans(starts);
    let floor = nav.floor_grid(scene.nav_settings.agent_radius, Some(&reach));
    Site::survey(scene, floor, None, 0.5).expect("a site")
}
