use glam::Vec3;

use super::super::super::super::test_support::{add_box, add_floor, assert_same_mesh, move_to};
use super::super::super::super::{NavSpan, NavigationGraph, Rebake, TileKey};
use rusty_scene::Scene;

fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

fn links(g: &NavigationGraph) -> Vec<String> {
    g.offmesh_links().map(|l| format!("{l:?}")).collect()
}

/// 16 m tiles of 32 columns; a crate in the middle of one, far from its borders.
fn level() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    scene.nav_settings.tile_size = 16.0;
    scene.nav_settings.drop_height = 2.0;
    add_floor(&mut scene, -20.0, 20.0, -20.0, 20.0);
    add_box(
        &mut scene,
        Vec3::new(-14.0, 0.0, 2.0),
        Vec3::new(-12.0, 1.2, 6.0),
    );
    let crate_id = add_box(
        &mut scene,
        Vec3::new(7.0, 0.0, 7.0),
        Vec3::new(9.0, 1.0, 9.0),
    );
    (scene, crate_id)
}

/// A tile's id range and spans: what a renumbering would change.
fn stored(g: &NavigationGraph, key: TileKey) -> (u32, Vec<NavSpan>) {
    let t = g.tiles.tile(key).expect("a stored tile");
    let ids = t.base as usize..(t.base + t.len()) as usize;
    (t.base, g.spans[ids].to_vec())
}

#[test]
fn growing_the_bounds_bakes_only_the_new_tiles_and_renumbers_none() {
    let (mut scene, _) = level();
    let mut g = baked(&scene);
    let (west, width, tiles) = (g.tile_of(0, 0), g.width, g.tile_count());
    let kept = stored(&g, west);
    add_floor(&mut scene, 20.0, 60.0, -20.0, 20.0);
    let Rebake::Incremental(rects) = g.sync(&scene) else {
        panic!("a bounds change keeps the tiles");
    };
    assert!(g.width > width && g.tile_count() > tiles, "the map grew");
    assert!(
        rects.iter().all(|r| r.x0 > width / 2),
        "rebaked only near the old east edge and past it: {rects:?}"
    );
    assert_eq!(stored(&g, west), kept, "the west tile keeps its ids");
    assert_same_mesh(&g, &baked(&scene));
    assert_eq!(links(&g), links(&baked(&scene)));
}

#[test]
fn growing_west_moves_every_cell_and_keeps_every_id() {
    let (mut scene, _) = level();
    let mut g = baked(&scene);
    let east = g.tile_of(g.width - 1, 0);
    let kept = stored(&g, east);
    add_floor(&mut scene, -60.0, -20.0, -20.0, 20.0);
    assert!(matches!(g.sync(&scene), Rebake::Incremental(_)));
    assert_eq!(
        g.tile_of(g.width - 1, 0),
        east,
        "the same key on the world lattice"
    );
    assert_eq!(stored(&g, east), kept);
    assert_same_mesh(&g, &baked(&scene));
    assert_eq!(links(&g), links(&baked(&scene)));
    assert!(
        g.changes_since(g.bake_generation - 1).is_none(),
        "cells moved: paths re-plan"
    );
}

#[test]
fn a_restore_rebakes_only_the_tile_whose_inputs_changed() {
    let (mut scene, crate_id) = level();
    let bytes = NavigationGraph::from_scene(&scene).bake_to_bytes(&scene);
    move_to(&mut scene, crate_id, Vec3::new(8.5, 0.5, 8.0));
    let mut g = NavigationGraph::from_scene(&scene);
    assert_eq!(g.restore(&scene, &bytes), Ok(1), "the crate's tile alone");
    assert_same_mesh(&g, &baked(&scene));
    assert_eq!(links(&g), links(&baked(&scene)));
}

#[test]
fn a_restore_under_grown_bounds_keeps_the_tiles_no_change_reaches() {
    let (mut scene, _) = level();
    let bytes = NavigationGraph::from_scene(&scene).bake_to_bytes(&scene);
    add_floor(&mut scene, 20.0, 60.0, -20.0, 20.0);
    let mut g = NavigationGraph::from_scene(&scene);
    let rebaked = g
        .restore(&scene, &bytes)
        .expect("the west tiles still match");
    assert!(
        rebaked > 0 && rebaked < g.tile_count(),
        "{rebaked} of {}",
        g.tile_count()
    );
    assert_same_mesh(&g, &baked(&scene));
    assert_eq!(links(&g), links(&baked(&scene)));
}

#[test]
fn rebakes_that_grow_tiles_never_let_dead_ids_outnumber_live_ones() {
    let (mut scene, _) = level();
    let slab = add_box(
        &mut scene,
        Vec3::new(-18.0, 2.5, -4.0),
        Vec3::new(-14.0, 2.7, 4.0),
    );
    let mut g = baked(&scene);
    let (mut moved, mut compacted) = (false, false);
    for step in 1..24 {
        let dead = g.tiles.dead;
        let x = -16.0 + 1.5 * step as f32;
        move_to(&mut scene, slab, Vec3::new(x, 2.6, 0.0));
        assert!(
            matches!(g.sync(&scene), Rebake::Incremental(_)),
            "step {step}"
        );
        assert!(!g.tiles.wasteful(&g.spans), "step {step}: compacted");
        moved |= g.tiles.dead > 0;
        compacted |= dead > 0 && g.tiles.dead == 0;
    }
    assert!(moved && compacted, "tiles moved and were compacted");
    assert_same_mesh(&g, &baked(&scene));
    let path = g.calculate_path(Vec3::new(-18.0, 0.0, -18.0), Vec3::new(18.0, 0.0, 18.0));
    assert_eq!(path.status, crate::NavPathStatus::Complete);
}
