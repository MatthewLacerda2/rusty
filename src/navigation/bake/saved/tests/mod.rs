//! Tests for the saved navmesh (#1887): it restores the graph a full bake gives,
//! is refused whole when stale or malformed, and a restored graph rebakes
//! incrementally like a baked one.

mod format;
mod fresh;
mod malformed;
mod restore;

use glam::Vec3;

use super::super::super::test_support::{add_box, add_floor, add_obstacle};
use super::super::super::NavigationGraph;
use crate::scene::Scene;

/// A floor with a raised deck (a drop link off its edge), two crates, a carving
/// obstacle and a modifier-free area table: every kind of saved data.
pub(super) fn level() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    scene.nav_settings.agent_radius = 0.25;
    scene.nav_settings.drop_height = 2.0;
    add_floor(&mut scene, 0.0, 12.0, 0.0, 8.0);
    add_box(
        &mut scene,
        Vec3::new(8.0, 0.0, 0.0),
        Vec3::new(12.0, 1.0, 8.0),
    );
    let crate_id = add_box(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(3.0, 1.0, 3.0),
    );
    add_box(
        &mut scene,
        Vec3::new(5.0, 0.0, 5.0),
        Vec3::new(6.0, 1.0, 6.0),
    );
    add_obstacle(&mut scene, Vec3::new(4.0, 0.5, 2.0), Vec3::ONE, true);
    (scene, crate_id)
}

/// `scene` baked from an unbaked graph.
pub(super) fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

/// `scene`'s saved bytes, and the graph that wrote them.
pub(super) fn saved(scene: &Scene) -> (NavigationGraph, Vec<u8>) {
    let mut g = NavigationGraph::from_scene(scene);
    let bytes = g.bake_to_bytes(scene);
    (g, bytes)
}
