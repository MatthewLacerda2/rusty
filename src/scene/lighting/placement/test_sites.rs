//! Primitive sites for #1737's acceptance tests: a yard, a house on it whose roof
//! no one reaches, and placement on a baked nav surface.

use glam::Vec3;

use super::test_level::block;
use super::{plan_reflection_probes, ReflectionPlacement, ReflectionPlan, Site};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// A 30 × 30 m yard, top at y = 0.
pub fn yard() -> Scene {
    let mut scene = Scene::new();
    block(
        &mut scene,
        Vec3::new(-15.0, -0.2, -15.0),
        Vec3::new(15.0, 0.0, 15.0),
    );
    scene
}

/// A 10 × 10 m room at the yard's middle, 4 m high, 2 m thick walls, a door on
/// −z, under a flat roof as wide as its walls: walkable, reached by nothing.
pub fn roofed_house(scene: &mut Scene) {
    let (o, i, h) = (7.0, 5.0, 4.0);
    block(scene, Vec3::new(-o, 0.0, i), Vec3::new(o, h, o));
    block(scene, Vec3::new(-o, 0.0, -i), Vec3::new(-i, h, i));
    block(scene, Vec3::new(i, 0.0, -i), Vec3::new(o, h, i));
    block(scene, Vec3::new(-o, 0.0, -o), Vec3::new(-1.5, h, -i));
    block(scene, Vec3::new(1.5, 0.0, -o), Vec3::new(o, h, -i));
    block(scene, Vec3::new(-o, h, -o), Vec3::new(o, h + 0.3, o));
}

/// `scene`'s reflection plans on its baked nav surface's reachable floor.
pub fn plan_on_nav(scene: &Scene) -> Vec<ReflectionPlan> {
    let site = site_on_nav(scene);
    plan_reflection_probes(scene, &site, &ReflectionPlacement::default(), &[])
}

/// `scene`'s site on its baked nav surface's reachable floor.
pub fn site_on_nav(scene: &Scene) -> Site {
    let nav = NavigationGraph::from_scene(scene);
    let reach = nav.reachable_spans(&[]);
    let floor = nav.floor_grid(scene.nav_settings.agent_radius, Some(&reach));
    Site::survey(scene, floor, None, 0.5).expect("a site")
}
