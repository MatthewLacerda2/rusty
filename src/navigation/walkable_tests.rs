//! Tests for `NavigationGraph::walkable_aabb` (`bounds.rs`, #721): the walkable
//! extent light-probe placement fills, and the probe grid it yields through
//! `scene::lighting::placement` — the same layout the graph-taking placement gave.

use super::test_support::{add_box, add_floor};
use super::*;
use crate::scene::lighting::placement::plan_light_probes;
use crate::scene::Scene;
use glam::Vec3;

/// A 20×6×20 room: a single static box spanning [-10,10] in XZ, [-3,3] in Y.
fn room_scene() -> Scene {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        Vec3::new(-10.0, -3.0, -10.0),
        Vec3::new(10.0, 3.0, 10.0),
    );
    scene
}

#[test]
fn a_flat_graph_bounds_its_cell_centres_at_floor_height() {
    let g = NavigationGraph::new(-4.0, 4.0, -4.0, 4.0, 1.0);
    let expected = (Vec3::new(-4.0, 0.0, -4.0), Vec3::new(4.0, 0.0, 4.0));
    assert_eq!(g.walkable_aabb(), Some(expected));
}

#[test]
fn a_degenerate_graph_has_no_walkable_extent() {
    assert_eq!(
        NavigationGraph::new(0.0, 0.0, 0.0, 0.0, 1.0).walkable_aabb(),
        None
    );
}

#[test]
fn a_graph_with_nothing_walkable_has_no_extent() {
    let g = NavigationGraph::from_scene(&Scene::new());
    assert_eq!(g.walkable_aabb(), None, "no geometry, no spans (#454)");
}

/// The bounds of a real bake: the slab's top, inset by the agent-radius erosion.
#[test]
fn a_baked_floor_bounds_its_eroded_surface() {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    let g = test_support::bake_pinned(&mut scene);
    let (min, max) = g.walkable_aabb().expect("the floor is walkable");
    assert_eq!((min.y, max.y), (0.0, 0.0), "floors at the slab's top");
    assert!(
        min.x > 0.0 && max.x < 10.0,
        "erosion pulls in off the world edge"
    );
}

/// The walkable extent handed to placement gives the layout the graph-taking
/// placement gave: XZ clipped to the level, Y from the floors up 3 units of headroom.
#[test]
fn probe_placement_through_walkable_bounds_is_unchanged() {
    let g = NavigationGraph::new(-4.0, 4.0, -4.0, 4.0, 1.0);
    let plan = plan_light_probes(&room_scene(), g.walkable_aabb(), 4.0).expect("a plan");
    assert_eq!(plan.min, Vec3::new(-4.0, 0.0, -4.0));
    assert_eq!(plan.max, Vec3::new(4.0, 3.0, 4.0));
    assert_eq!(plan.spacing, 4.0);
}

/// Agent-radius erosion (#277) must TIGHTEN — never grow — the probe bounds: probes
/// track where agents can actually stand.
#[test]
fn erosion_tightens_light_probe_bounds() {
    let bake_with = |radius: f32| {
        let mut scene = room_scene();
        scene.nav_settings.agent_radius = radius;
        scene.nav_settings.bounds = Some(NavBounds::new(-10.0, 10.0, -10.0, 10.0));
        let mut nav = NavigationGraph::new(-10.0, 10.0, -10.0, 10.0, 1.0);
        nav.bake(&scene);
        plan_light_probes(&scene, nav.walkable_aabb(), 4.0).expect("a plan from the bake")
    };
    let (wide, eroded) = (bake_with(0.0), bake_with(2.0));
    let (ws, es) = (wide.max - wide.min, eroded.max - eroded.min);
    assert!(es.x <= ws.x + 1e-4 && es.z <= ws.z + 1e-4, "never grows");
    assert!(
        es.x < ws.x - 1e-4 || es.z < ws.z - 1e-4,
        "a 2-unit radius tightens"
    );
}
