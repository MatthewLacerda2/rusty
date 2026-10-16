//! The windowed boot and the headless harness bake the same navmesh (#452).
//!
//! Both used to hardcode a ±20 grid; both now go through `NavigationGraph::from_scene`.
//! The harness's graph must match a fresh `from_scene` bake of its scene — the default
//! scene the windowed game boots (#667) — and that bake spans the scene's own bounds.

use rusty::dev::harness::Harness;
use rusty::navigation::{NavBounds, NavigationGraph};
use rusty::scene::Scene;

#[test]
fn harness_graph_is_the_from_scene_bake_of_its_scene() {
    let h = Harness::new(crate::temp::dir().join("rusty_test_nav_bounds"), "");
    let world = h.world.borrow();
    let nav = world.nav().borrow();

    let mut demo = Scene::new();
    rusty::scene::default_scene::build(&mut demo, "");
    let fresh = NavigationGraph::from_scene(&demo);

    assert_eq!(nav.bounds(), fresh.bounds());
    assert_eq!(nav.spans, fresh.spans);
    assert_eq!(nav.cell_start, fresh.cell_start);
    assert!(!nav.spans.is_empty(), "the yard is walkable");
    // The yard spans ±15 × ±22.5; + 2.0 margin + 0.5 radius, snapped outward.
    assert_eq!(nav.bounds(), NavBounds::new(-18.0, 18.0, -25.0, 25.0));
}

#[test]
fn default_scene_bakes_over_its_own_bounds() {
    let mut scene = Scene::new();
    rusty::scene::default_scene::build(&mut scene, "");
    let nav = NavigationGraph::from_scene(&scene);
    let b = nav.bounds();
    eprintln!("default scene bounds: {b:?}");
    assert!(b.is_valid());
    assert_ne!(
        b,
        NavBounds::EMPTY_SCENE,
        "derived from geometry, not the fallback"
    );
}
