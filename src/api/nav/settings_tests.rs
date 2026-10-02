//! Faithfulness tests for the `Navigation` per-scene bake settings API (#276).
//!
//! Each setter must prove its write is *consumed*: we set via the Lua binding, assert
//! `scene.nav_settings` reflects it, and that a re-bake observes it — the bake is the
//! read-site. `agent_radius`'s bake effect (walkable-surface erosion) is exercised in the
//! navigation tests (`navigation::bake::tests::erosion`); here we assert it persists/round-trips.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::register;
use crate::navigation::test_support::{add_box, add_floor, ground};
use crate::navigation::{NavBounds, NavigationGraph};
use crate::scene::Scene;

/// A scene with a floor (top y = 0) and one static box collider spanning the given world
/// AABB, plus a freshly baked graph over a 0..10 × 0..10 grid at unit spacing.
fn scene_and_nav(min: Vec3, max: Vec3) -> (RefCell<Scene>, RefCell<NavigationGraph>) {
    let mut scene = Scene::new();
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(&mut scene, min, max);
    // Pin the grid (#452) so the cell indices asserted below stay put.
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 10.0, 0.0, 10.0));
    let mut nav = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    nav.bake(&scene);
    (RefCell::new(scene), RefCell::new(nav))
}

/// Whether the ledge on cell (4,4) is one step from the floor at (3,4).
fn ledge_reachable(nav: &RefCell<NavigationGraph>) -> bool {
    let nav = nav.borrow();
    nav.link_to(ground(&nav, 3, 4), 4, 4).is_some()
}

/// `SetMaxStep` round-trips into `scene.nav_settings` AND is consumed by the re-bake:
/// a 1.0-high ledge cell is reachable from ground when max_step ≥ 1.0 but not below it,
/// proving the bake sources `scene.nav_settings`, not the historical constant (0.5).
#[test]
fn set_max_step_round_trips_and_rebakes() {
    // A box [3.6,4.4]² to y=1.0 covers exactly cell (4,4); ground is y=0 around it.
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 1.0, 4.4));
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();

        // Default max_step (0.5): a 1.0 ledge is NOT reachable from a neighbour.
        let before: f32 = lua.load("return Navigation.GetMaxStep()").eval().unwrap();
        assert_eq!(before, 0.5, "starts at the historical default");
        assert!(!ledge_reachable(&nav), "1.0 ledge blocked at step 0.5");

        // Raise max_step past the ledge: the setter re-bakes, so it becomes reachable.
        lua.load("Navigation.SetMaxStep(1.5)").exec().unwrap();
        let after: f32 = lua.load("return Navigation.GetMaxStep()").eval().unwrap();
        assert_eq!(after, 1.5, "getter reflects the write");
        Ok(())
    })
    .unwrap();

    assert_eq!(
        scene.borrow().nav_settings.max_step,
        1.5,
        "scene settings updated"
    );
    assert_eq!(
        nav.borrow().max_step,
        1.5,
        "re-bake pushed it into the graph"
    );
    assert!(
        ledge_reachable(&nav),
        "1.0 ledge reachable after raising max_step to 1.5"
    );
}

/// `SetGridSpacing` re-shapes the grid on re-bake: a coarser spacing yields fewer cells.
#[test]
fn set_grid_spacing_reshapes_grid_on_rebake() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 0.2, 4.4));
    let cells_before = nav.borrow().cell_start.len();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        lua.load("Navigation.SetGridSpacing(2.0)").exec().unwrap();
        Ok(())
    })
    .unwrap();

    assert_eq!(scene.borrow().nav_settings.grid_spacing, 2.0);
    assert_eq!(nav.borrow().grid_spacing, 2.0, "graph re-spaced on re-bake");
    assert!(
        nav.borrow().cell_start.len() < cells_before,
        "coarser spacing => fewer cells"
    );
}

/// `SetAgentHeight` round-trips into `scene.nav_settings` AND is consumed by the re-bake
/// (#278): a low overhang (underside 1.0 above the floor) leaves the cell beneath it
/// walkable at a short agent height but carves it once the agent height is raised past the
/// clearance — proving the bake's headroom pass is the read-site, via the binding.
#[test]
fn set_agent_height_round_trips_and_rebakes() {
    // A slab over cell (4,4): footprint [3.6,4.4]², underside y=1.0 (above the y=0 floor).
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 1.0, 3.6), Vec3::new(4.4, 1.5, 4.4));
    // Keep radius 0 so erosion doesn't blur the single carved cell we assert on.
    scene.borrow_mut().nav_settings.agent_radius = 0.0;
    nav.borrow_mut().bake(&scene.borrow());

    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();

        // Short agent (0.5 < 1.0 clearance): the cell under the overhang stays walkable.
        lua.load("Navigation.SetAgentHeight(0.5)").exec().unwrap();
        assert!(
            nav.borrow().spans_at(4, 4)[0].y == 0.0,
            "0.5 agent fits under a 1.0 overhang"
        );

        // Tall agent (2.0 > 1.0 clearance): the setter re-bakes and carves the cell.
        lua.load("Navigation.SetAgentHeight(2.0)").exec().unwrap();
        let h: f32 = lua
            .load("return Navigation.GetAgentHeight()")
            .eval()
            .unwrap();
        assert_eq!(h, 2.0, "getter reflects the write");
        assert!(
            nav.borrow().spans_at(4, 4)[0].y > 0.0,
            "2.0 agent cannot fit under a 1.0 overhang — floor dropped on re-bake"
        );
        Ok(())
    })
    .unwrap();

    assert_eq!(
        scene.borrow().nav_settings.agent_height,
        2.0,
        "scene settings updated"
    );
}

/// `SetAgentRadius`/`SetMaxSlope` round-trip into the scene settings via the bindings.
/// Radius is stored-but-inert (#276), so this is the persistence proof the bake doesn't
/// yet consume it — only that the write survives.
#[test]
fn set_radius_and_slope_round_trip() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 0.2, 4.4));
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        lua.load("Navigation.SetAgentRadius(0.75)").exec().unwrap();
        lua.load("Navigation.SetMaxSlope(2.0)").exec().unwrap();
        let r: f32 = lua
            .load("return Navigation.GetAgentRadius()")
            .eval()
            .unwrap();
        let s: f32 = lua.load("return Navigation.GetMaxSlope()").eval().unwrap();
        assert_eq!(r, 0.75);
        assert_eq!(s, 2.0);
        Ok(())
    })
    .unwrap();

    let s = scene.borrow();
    assert_eq!(s.nav_settings.agent_radius, 0.75, "radius persists");
    assert_eq!(s.nav_settings.max_slope, 2.0);
    assert_eq!(
        nav.borrow().max_slope,
        2.0,
        "slope reached the graph via re-bake"
    );
}

/// `SetBounds` authors the override and re-bakes onto it; `GetBounds` reads the baked
/// (snapped) bounds plus the authored flag; `ClearBounds` returns to the derived ones
/// (#452). An inverted box is a Lua error and leaves the scene untouched.
#[test]
fn set_get_clear_bounds_round_trip_and_rebake() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 1.0, 4.4));
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        let get = || -> (f32, f32, f32, f32, bool) {
            lua.load("return Navigation.GetBounds()").eval().unwrap()
        };
        assert_eq!(get(), (0.0, 10.0, 0.0, 10.0, true), "pinned by the fixture");

        lua.load("Navigation.SetBounds(-60.5, 70, -3, 3.2)")
            .exec()
            .unwrap();
        assert_eq!(get(), (-61.0, 70.0, -3.0, 4.0, true), "snapped outward");
        assert_eq!((nav.borrow().width, nav.borrow().height), (132, 8));

        let bad = lua.load("Navigation.SetBounds(5, 5, 0, 1)").exec();
        assert!(bad.is_err(), "empty extent rejected");
        assert_eq!(get().0, -61.0, "rejected write left the bounds alone");

        lua.load("Navigation.ClearBounds()").exec().unwrap();
        // Derived: [3.6, 4.4] grown by 2.0 + 0.5 → [1.1, 6.9] → [1, 7].
        assert_eq!(get(), (1.0, 7.0, 1.0, 7.0, false));
        Ok(())
    })
    .unwrap();
    assert_eq!(scene.borrow().nav_settings.bounds, None);
}
