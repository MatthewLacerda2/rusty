//! The bake reads its tunables from `scene.nav_settings` (#276).

use super::super::super::test_support::{add_box, add_floor, bake_pinned, ground};
use super::super::super::{NavBounds, NavigationGraph};
use crate::scene::Scene;
use glam::Vec3;

#[test]
fn bake_sources_max_step_from_scene_settings() {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(
        &mut scene,
        Vec3::new(3.6, 0.0, 3.6),
        Vec3::new(4.4, 1.0, 4.4),
    );
    scene.nav_settings.agent_radius = 0.0;
    let mut g = bake_pinned(&mut scene);
    assert!(
        g.link_to(ground(&g, 3, 4), 4, 4).is_none(),
        "1.0 ledge at step 0.5"
    );
    scene.nav_settings.max_step = 1.5;
    g.bake(&scene);
    assert_eq!(g.max_step, 1.5);
    assert!(
        g.link_to(ground(&g, 3, 4), 4, 4).is_some(),
        "reachable at step 1.5"
    );
}

#[test]
fn bake_reshapes_grid_from_grid_spacing() {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 10.0, 0.0, 10.0));
    let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    g.bake(&scene);
    assert_eq!((g.width, g.height), (11, 11));
    scene.nav_settings.grid_spacing = 2.0;
    g.bake(&scene);
    assert_eq!((g.grid_spacing, g.width, g.height), (2.0, 6, 6));
    assert_eq!(g.cell_start.len(), 37);
}
