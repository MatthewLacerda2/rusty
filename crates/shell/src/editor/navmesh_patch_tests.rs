//! The NavMesh gizmo's patching (#1910): a small rebake re-uploads only the tiles
//! it touched, and the patched tiles are what a full rebuild would give.

use glam::Vec3;

use super::*;
use rusty_navigation::test_support::{add_box, add_floor, move_to};
use rusty_navigation::{NavBounds, Rebake};
use rusty_scene::Scene;

/// A Dust2-sized level (200 m square at the default 0.25 m cells) with one small
/// crate, baked; the crate's id.
fn dust2() -> (Scene, NavigationGraph, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(-100.0, 100.0, -100.0, 100.0));
    add_floor(&mut scene, -100.0, 100.0, -100.0, 100.0);
    let id = add_box(&mut scene, Vec3::ZERO, Vec3::new(0.2, 0.5, 0.2));
    let mut g = NavigationGraph::from_scene(&scene);
    g.bake(&scene);
    (scene, g, id)
}

/// Every tile, built in full.
fn all_tiles(g: &NavigationGraph) -> Vec<Vec<Vec3>> {
    let plan = plan(None, g).expect("a first plan");
    plan.tiles.iter().map(|&t| tile_triangles(g, t)).collect()
}

/// Moving the crate one cell rebakes a few cells: only the tiles they reach are
/// rebuilt, their bytes a sliver of the whole, and the patched set equals a full
/// rebuild of the new graph.
#[test]
fn a_one_cell_rebake_rewrites_only_its_tiles() {
    let (mut scene, mut g, id) = dust2();
    let mut tiles = all_tiles(&g);
    let shown = plan(None, &g).unwrap().revision;
    move_to(&mut scene, id, Vec3::new(0.35, 0.25, 0.1));
    let Rebake::Incremental(rects) = g.sync(&scene) else {
        panic!("a moved crate rebakes incrementally");
    };
    let plan = plan(Some(shown), &g).expect("the graph changed");
    assert_eq!(plan.reset, None, "patched, not started over");
    let across = tiles_across(g.width);
    let reached = |t: usize| {
        let (tx, tz) = ((t as i32 % across) * TILE, (t as i32 / across) * TILE);
        let within = |lo: i32, hi: i32, t0: i32| lo < t0 + TILE && hi >= t0;
        rects
            .iter()
            .any(|r| within(r.x0, r.x1, tx) && within(r.z0, r.z1, tz))
    };
    assert!(
        !plan.tiles.is_empty() && plan.tiles.len() <= 4,
        "{:?}",
        plan.tiles
    );
    assert!(
        plan.tiles.iter().all(|&t| reached(t)),
        "only tiles a rect reaches"
    );
    let mut written = 0;
    for &t in &plan.tiles {
        tiles[t] = tile_triangles(&g, t);
        written += tiles[t].len() * 12;
    }
    let whole: usize = tiles.iter().map(|t| t.len() * 12).sum();
    assert!(
        written * 100 < whole,
        "{written} of {whole} bytes rewritten"
    );
    assert_eq!(tiles, all_tiles(&g), "the patch is a full rebuild's result");
    assert!(
        super::plan(Some(plan.revision), &g).is_none(),
        "then up to date"
    );
}

/// An area-cost edit or a full rebake can't be answered from the change log: the
/// whole navmesh is rebuilt.
#[test]
fn a_full_bake_starts_over() {
    let (scene, mut g, _) = dust2();
    let shown = plan(None, &g).unwrap().revision;
    g.bake(&scene);
    let plan = plan(Some(shown), &g).expect("rebaked");
    assert_eq!(plan.reset, Some(plan.tiles.len()));
    assert_eq!(plan.tiles.len(), (tiles_across(g.width) as usize).pow(2));
}
