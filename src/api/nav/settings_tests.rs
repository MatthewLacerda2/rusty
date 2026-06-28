//! Faithfulness tests for the `Navigation` per-scene bake settings API (#276).
//!
//! Each setter must prove its write is *consumed*: we set via the Lua binding, assert
//! `scene.nav_settings` reflects it, and (for the live knobs) that a re-bake observes it
//! — the bake is the read-site. `agent_radius` is stored-but-inert, so it only needs a
//! persistence/round-trip assertion.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::register;
use crate::components::{ColliderComponent, ColliderShape};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// A scene with one static box collider spanning the given world AABB, plus a freshly
/// baked graph over a 0..10 × 0..10 grid at unit spacing.
fn scene_and_nav(min: Vec3, max: Vec3) -> (RefCell<Scene>, RefCell<NavigationGraph>) {
    let mut scene = Scene::new();
    let id = scene.add_entity("step".to_string());
    {
        let mut e = scene.get_entity_mut(id).expect("entity exists");
        e.is_static = true;
        e.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size: max - min },
            is_trigger: false,
            aabb_min: min,
            aabb_max: max,
        });
    }
    let mut nav = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    nav.bake(&scene);
    (RefCell::new(scene), RefCell::new(nav))
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
        assert!(
            !nav.borrow().cell_reachable(4, 4),
            "1.0 ledge blocked at step 0.5"
        );

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
        nav.borrow().cell_reachable(4, 4),
        "1.0 ledge reachable after raising max_step to 1.5"
    );
}

/// `SetGridSpacing` re-shapes the grid on re-bake: a coarser spacing yields fewer cells.
#[test]
fn set_grid_spacing_reshapes_grid_on_rebake() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 0.2, 4.4));
    let cells_before = nav.borrow().walkability.len();
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
        nav.borrow().walkability.len() < cells_before,
        "coarser spacing => fewer cells"
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
    assert_eq!(s.nav_settings.agent_radius, 0.75, "radius persists (inert)");
    assert_eq!(s.nav_settings.max_slope, 2.0);
    assert_eq!(
        nav.borrow().max_slope,
        2.0,
        "slope reached the graph via re-bake"
    );
}
