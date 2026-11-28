use glam::Vec3;

use super::super::super::test_support::{add_box, add_floor, assert_same_mesh};
use super::super::super::{NavPathStatus, NavigationGraph, OffMeshLinkKind};
use rusty_scene::Scene;

fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

fn links(g: &NavigationGraph) -> Vec<String> {
    g.offmesh_links().map(|l| format!("{l:?}")).collect()
}

/// Dust2-sized: 200 m square at the default 0.25 m cells, blocks scattered over tile borders and
/// a deck 1.5 m up crossing one, with drops off its sides.
fn arena(tile_size: f32) -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.tile_size = tile_size;
    scene.nav_settings.drop_height = 2.0;
    add_floor(&mut scene, -100.0, 100.0, -100.0, 100.0);
    for i in 0..48 {
        let (x, z) = ((i * 37 % 180) as f32 - 90.0, (i * 53 % 180) as f32 - 90.0);
        let top = 0.6 + (i % 3) as f32;
        add_box(
            &mut scene,
            Vec3::new(x, 0.0, z),
            Vec3::new(x + 3.0, top, z + 2.0),
        );
    }
    add_box(
        &mut scene,
        Vec3::new(20.0, 0.0, -10.0),
        Vec3::new(40.0, 1.5, 30.0),
    );
    scene
}

#[test]
fn a_tiled_bake_answers_every_path_query_as_the_whole_grid_bake_does() {
    let tiled = baked(&arena(32.0));
    let whole = baked(&arena(1024.0));
    assert!(tiled.tile_count() > 30, "{} tiles", tiled.tile_count());
    assert_eq!(
        whole.tile_count(),
        4,
        "the four tiles meeting at the origin hold it all"
    );
    assert_same_mesh(&tiled, &whole);
    assert_eq!(links(&tiled), links(&whole));
    let points = [
        Vec3::new(-95.0, 0.0, -95.0),
        Vec3::new(95.0, 0.0, 95.0),
        Vec3::new(30.0, 1.5, 10.0),
        Vec3::new(-31.9, 0.0, 32.1),
        Vec3::new(64.2, 0.0, -63.8),
        Vec3::new(0.0, 0.0, 0.0),
    ];
    for &a in &points {
        for &b in &points {
            let (t, w) = (tiled.calculate_path(a, b), whole.calculate_path(a, b));
            assert_eq!(format!("{t:?}"), format!("{w:?}"), "{a} -> {b}");
        }
    }
}

/// An open floor across the border between tiles 0 and 1 at x = 32 m.
fn strip() -> Scene {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 64.0, 0.0, 10.0);
    scene
}

#[test]
fn an_agent_crosses_a_tile_border_on_a_straight_path() {
    let g = baked(&strip());
    let (from, to) = (Vec3::new(10.0, 0.0, 5.0), Vec3::new(54.0, 0.0, 5.0));
    let (a, b) = (g.world_to_grid(from), g.world_to_grid(to));
    assert_ne!(g.tile_of(a.0, a.1), g.tile_of(b.0, b.1), "two tiles");
    let path = g.calculate_path(from, to);
    assert_eq!(path.status, NavPathStatus::Complete);
    assert_eq!(path.corners, vec![from, to], "no corner at the border");
}

#[test]
fn a_generated_drop_link_spans_two_tiles() {
    let mut scene = Scene::new();
    scene.nav_settings.drop_height = 2.0;
    add_floor(&mut scene, 16.0, 48.0, 0.0, 10.0);
    add_box(
        &mut scene,
        Vec3::new(16.0, 0.0, 0.0),
        Vec3::new(31.6, 1.5, 10.0),
    );
    let g = baked(&scene);
    let tile = |e: crate::LinkEnd| g.tile_of(e.gx, e.gz);
    let across = g
        .offmesh_links()
        .filter(|l| l.kind == OffMeshLinkKind::Drop);
    assert!(
        across.into_iter().any(|l| tile(l.from) != tile(l.to)),
        "a drop from the deck in one tile to the floor in the next"
    );
    let path = g.calculate_path(Vec3::new(20.0, 1.5, 5.0), Vec3::new(44.0, 0.0, 5.0));
    assert_eq!(path.status, NavPathStatus::Complete);
    assert!(!path.links.is_empty(), "it takes the drop");
    scene.nav_settings.tile_size = 1024.0;
    assert_eq!(
        links(&g),
        links(&baked(&scene)),
        "the links a whole-grid bake makes"
    );
}

#[test]
fn removing_a_tile_leaves_its_neighbours_walkable_up_to_the_border() {
    let mut g = baked(&strip());
    let cells = |g: &NavigationGraph| {
        let x = (0..g.width).filter(|&gx| g.tile_of(gx, 0).0 == 0);
        let column = |gx| (0..g.height).map(move |gz| g.spans_at(gx, gz).to_vec());
        x.flat_map(column).collect::<Vec<_>>()
    };
    let kept = cells(&g);
    let (gx, gz) = g.world_to_grid(Vec3::new(40.0, 0.0, 5.0));
    assert!(g.remove_tile(g.tile_of(gx, gz)), "the tile held spans");
    assert!(g.spans_at(gx, gz).is_empty());
    assert_eq!(cells(&g), kept, "tile 0 is untouched");
    let edge = Vec3::new(31.75, 0.0, 5.0);
    assert_eq!(
        g.tile_of(g.world_to_grid(edge).0, 0).0,
        0,
        "the last column of tile 0"
    );
    let to_edge = g.calculate_path(Vec3::new(5.0, 0.0, 5.0), edge);
    assert_eq!(to_edge.status, NavPathStatus::Complete);
    assert_eq!(
        to_edge.corners.last(),
        Some(&edge),
        "walkable up to the border"
    );
    let past = g.calculate_path(Vec3::new(5.0, 0.0, 5.0), Vec3::new(40.0, 0.0, 5.0));
    assert_ne!(past.status, NavPathStatus::Complete, "nothing past it");
}
