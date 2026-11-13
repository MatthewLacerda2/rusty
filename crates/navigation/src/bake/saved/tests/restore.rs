//! Restoring: the same graph a bake gives, refused when stale, incremental after.

use glam::Vec3;

use super::super::super::super::test_support::{assert_same_mesh, move_to};
use super::super::super::super::{NavigationGraph, Rebake, Start};
use super::{baked, level, saved};

/// Every generated link, as comparable values.
fn links(g: &NavigationGraph) -> Vec<String> {
    g.offmesh_links().map(|l| format!("{l:?}")).collect()
}

#[test]
fn a_restore_is_the_graph_a_full_bake_gives() {
    let (scene, _) = level();
    let (written, bytes) = saved(&scene);
    assert!(
        written.offmesh_links().next().is_some(),
        "the level has links"
    );
    let mut g = NavigationGraph::from_scene(&scene);
    g.restore(&scene, &bytes).expect("a fresh file matches");
    assert_same_mesh(&g, &baked(&scene));
    let surfaces = |g: &NavigationGraph| format!("{:?}", g.spans);
    assert_eq!(surfaces(&g), surfaces(&baked(&scene)), "surfaces too");
    assert_eq!(links(&g), links(&baked(&scene)));
    assert_eq!(
        (g.width, g.height, g.grid_spacing),
        (written.width, written.height, 0.5)
    );
}

#[test]
fn a_stale_file_is_refused_and_the_graph_left_as_it_was() {
    let (mut scene, crate_id) = level();
    let (_, bytes) = saved(&scene);
    move_to(&mut scene, crate_id, Vec3::new(1.0, 0.5, 6.0));
    let mut g = NavigationGraph::new(0.0, 3.0, 0.0, 3.0, 1.0);
    let err = g.restore(&scene, &bytes).expect_err("the crate moved");
    assert!(err.contains("changed"), "{err}");
    assert_eq!((g.width, g.spans.len(), g.bake_generation), (4, 16, 0));
    let mut settings = level().0;
    settings.nav_settings.agent_height = 1.5;
    assert!(
        g.restore(&settings, &bytes).is_err(),
        "a bake setting changed"
    );
    let mut spacing = level().0;
    spacing.nav_settings.grid_spacing = 0.25;
    assert!(g.restore(&spacing, &bytes).is_err(), "another grid");
}

#[test]
fn an_area_cost_change_keeps_the_file() {
    let (mut scene, _) = level();
    let (_, bytes) = saved(&scene);
    scene.nav_settings.areas[0].cost = 3.0;
    let mut g = NavigationGraph::from_scene(&scene);
    g.restore(&scene, &bytes)
        .expect("costs are read at search time");
    assert_eq!(g.area_costs[0], 3.0);
}

/// The issue's check: after a restore, moving a static collider rebakes only its
/// cells, and the result is the graph a full bake of the moved scene gives.
#[test]
fn a_restored_graph_rebakes_only_what_changed() {
    let (mut scene, crate_id) = level();
    let (_, bytes) = saved(&scene);
    let mut g = NavigationGraph::from_scene(&scene);
    g.restore(&scene, &bytes).unwrap();
    assert_eq!(
        g.sync(&scene),
        Rebake::Unchanged,
        "the record matches the scene"
    );
    move_to(&mut scene, crate_id, Vec3::new(2.5, 0.5, 6.0));
    let Rebake::Incremental(rects) = g.sync(&scene) else {
        panic!("a moved crate rebakes incrementally");
    };
    let cells: usize = rects.iter().map(|r| r.cells()).sum();
    assert!(cells < (g.width * g.height) as usize / 2, "{cells} cells");
    assert_same_mesh(&g, &baked(&scene));
    assert_eq!(links(&g), links(&baked(&scene)));
}

/// Play's first tick takes the saved navmesh when it matches, and bakes when not.
#[test]
fn start_tracked_restores_a_matching_file_and_bakes_a_stale_one() {
    let (mut scene, crate_id) = level();
    let (_, bytes) = saved(&scene);
    scene.navmesh.bytes = Some(bytes.into());
    let mut g = NavigationGraph::from_scene(&scene);
    assert_eq!(g.start_tracked(&mut scene), Start::Restored);
    assert!(g.tracks(&scene));
    move_to(&mut scene, crate_id, Vec3::new(1.0, 0.5, 6.0));
    let mut fresh = NavigationGraph::from_scene(&scene);
    assert_eq!(fresh.start_tracked(&mut scene), Start::Baked);
    assert_same_mesh(&fresh, &baked(&scene));
}

/// A bake that re-shapes the grid during Play (`Navigation.SetGridSpacing`,
/// `Navigation.Bake`) keeps the play loop's tracking, so the next tick syncs
/// rather than refusing the saved navmesh and baking again.
#[test]
fn a_reshaping_bake_keeps_tracking_the_scene() {
    let (mut scene, _) = level();
    let mut g = NavigationGraph::from_scene(&scene);
    g.start_tracked(&mut scene);
    scene.nav_settings.grid_spacing = 0.25;
    g.bake(&scene);
    assert_eq!(g.grid_spacing, 0.25, "re-shaped");
    assert!(g.tracks(&scene));
}

/// Zero: an empty scene's navmesh (no span, no link) saves and restores too.
#[test]
fn an_empty_scene_round_trips() {
    let scene = rusty_scene::Scene::new();
    let (written, bytes) = saved(&scene);
    assert!(written.spans.is_empty());
    let mut g = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 1.0);
    g.restore(&scene, &bytes).expect("an empty bake matches");
    assert!(g.spans.is_empty());
    assert_eq!((g.width, g.height), (written.width, written.height));
}
