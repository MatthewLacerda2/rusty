use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::super::register;
use crate::api::bind;
use rusty_navigation::test_support::{add_box, add_floor, ground};
use rusty_navigation::{NavBounds, NavigationGraph};
use rusty_scene::Scene;

fn scene_and_nav(min: Vec3, max: Vec3) -> (Rc<RefCell<Scene>>, Rc<RefCell<NavigationGraph>>) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 1.0;
    add_floor(&mut scene, 0.0, 10.0, 0.0, 10.0);
    add_box(&mut scene, min, max);
    // Pinned bounds keep the asserted cell indices put; radius 0 keeps erosion off those cells.
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 10.0, 0.0, 10.0));
    scene.nav_settings.agent_radius = 0.0;
    let mut nav = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    nav.bake(&scene);
    (Rc::new(RefCell::new(scene)), Rc::new(RefCell::new(nav)))
}

fn ledge_reachable(nav: &Rc<RefCell<NavigationGraph>>) -> bool {
    let nav = nav.borrow();
    nav.link_to(ground(&nav, 3, 4), 4, 4).is_some()
}

#[test]
fn set_max_step_round_trips_and_rebakes() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 1.0, 4.4));
    let lua = Lua::new();
    register(&lua, bind(&lua, &scene), bind(&lua, &nav)).unwrap();

    let before: f32 = lua.load("return Navigation.GetMaxStep()").eval().unwrap();
    assert_eq!(before, 0.3, "starts at the default");
    assert!(!ledge_reachable(&nav), "1.0 ledge blocked at step 0.3");

    lua.load("Navigation.SetMaxStep(1.5)").exec().unwrap();
    let after: f32 = lua.load("return Navigation.GetMaxStep()").eval().unwrap();
    assert_eq!(after, 1.5, "getter reflects the write");

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

#[test]
fn set_grid_spacing_reshapes_grid_on_rebake() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 0.2, 4.4));
    let cells_before = nav.borrow().cell_start.len();
    let lua = Lua::new();
    register(&lua, bind(&lua, &scene), bind(&lua, &nav)).unwrap();
    lua.load("Navigation.SetGridSpacing(2.0)").exec().unwrap();

    assert_eq!(scene.borrow().nav_settings.grid_spacing, 2.0);
    assert_eq!(nav.borrow().grid_spacing, 2.0, "graph re-spaced on re-bake");
    assert!(
        nav.borrow().cell_start.len() < cells_before,
        "coarser spacing => fewer cells"
    );
}

#[test]
fn set_agent_height_round_trips_and_rebakes() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 1.0, 3.6), Vec3::new(4.4, 1.5, 4.4));
    scene.borrow_mut().nav_settings.agent_radius = 0.0;
    nav.borrow_mut().bake(&scene.borrow());

    let lua = Lua::new();
    register(&lua, bind(&lua, &scene), bind(&lua, &nav)).unwrap();

    lua.load("Navigation.SetAgentHeight(0.5)").exec().unwrap();
    assert!(
        nav.borrow().spans_at(4, 4)[0].y == 0.0,
        "0.5 agent fits under a 1.0 overhang"
    );

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

    assert_eq!(
        scene.borrow().nav_settings.agent_height,
        2.0,
        "scene settings updated"
    );
}

#[test]
fn set_radius_and_slope_round_trip() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 0.2, 4.4));
    let lua = Lua::new();
    register(&lua, bind(&lua, &scene), bind(&lua, &nav)).unwrap();
    lua.load("Navigation.SetAgentRadius(0.75)").exec().unwrap();
    lua.load("Navigation.SetMaxSlope(2.0)").exec().unwrap();
    let r: f32 = lua
        .load("return Navigation.GetAgentRadius()")
        .eval()
        .unwrap();
    let s: f32 = lua.load("return Navigation.GetMaxSlope()").eval().unwrap();
    assert_eq!(r, 0.75);
    assert_eq!(s, 2.0);

    let s = scene.borrow();
    assert_eq!(s.nav_settings.agent_radius, 0.75, "radius persists");
    assert_eq!(s.nav_settings.max_slope, 2.0);
    assert_eq!(
        nav.borrow().max_slope,
        2.0,
        "slope reached the graph via re-bake"
    );
}

#[test]
fn set_get_clear_bounds_round_trip_and_rebake() {
    let (scene, nav) = scene_and_nav(Vec3::new(3.6, 0.0, 3.6), Vec3::new(4.4, 1.0, 4.4));
    let lua = Lua::new();
    register(&lua, bind(&lua, &scene), bind(&lua, &nav)).unwrap();
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
    // Derived: the floor's [0, 10] grown by `BOUNDS_MARGIN` (2 m) plus the 0 m radius.
    assert_eq!(get(), (-2.0, 12.0, -2.0, 12.0, false));
    assert_eq!(scene.borrow().nav_settings.bounds, None);
}
