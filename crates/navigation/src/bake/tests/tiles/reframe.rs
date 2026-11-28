//! Tiles kept or restored under other bounds are the tiles a fresh bake gives.

use glam::{Quat, Vec3};

use super::super::super::super::test_support::{add_floor, add_rotated_box, assert_same_mesh};
use super::super::super::super::{NavBounds, NavigationGraph, Rebake};
use rusty_scene::Scene;

fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

/// Every span's bits, its surface plane included.
fn spans(g: &NavigationGraph) -> Vec<String> {
    let cells = (0..g.height).flat_map(|gz| (0..g.width).map(move |gx| (gx, gz)));
    cells
        .map(|(gx, gz)| format!("{:?}", g.spans_at(gx, gz)))
        .collect()
}

#[test]
fn a_restore_after_the_bounds_shrink_onto_a_tile_border_erodes_the_new_edge() {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    add_floor(&mut scene, -16.0, 32.0, 0.0, 16.0);
    scene.nav_settings.bounds = Some(NavBounds::new(-16.0, 32.0, -2.0, 18.0));
    let bytes = NavigationGraph::from_scene(&scene).bake_to_bytes(&scene);
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 32.0, -2.0, 18.0));
    let mut g = NavigationGraph::from_scene(&scene);
    let rebaked = g.restore(&scene, &bytes).expect("a tile still matches");
    assert!(rebaked > 0, "the tile on the new edge is rebaked");
    assert_same_mesh(&g, &baked(&scene));
    assert!(!g.is_walkable(0, 20), "the west edge is eroded");
}

#[test]
fn tiles_kept_across_a_reframe_match_a_fresh_bake_at_a_spacing_of_a_tenth() {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.1;
    scene.nav_settings.tile_size = 4.0;
    add_floor(&mut scene, 0.0, 12.0, 0.0, 6.0);
    let tilt = Quat::from_rotation_z(0.2);
    let ramp = Vec3::new(2.0, 0.2, 0.1);
    add_rotated_box(&mut scene, Vec3::new(8.3, 0.1, 3.0), ramp, tilt, true);
    let mut g = baked(&scene);
    add_floor(&mut scene, -10.0, 0.0, 0.0, 6.0);
    assert!(matches!(g.sync(&scene), Rebake::Incremental(_)));
    let fresh = baked(&scene);
    assert_same_mesh(&g, &fresh);
    assert_eq!(spans(&g), spans(&fresh), "the ramp's planes too");
}
