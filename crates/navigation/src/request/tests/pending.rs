//! `PathPending` (#2087) after a rebake that left the path alone and once no path
//! can be planned, and requests the queue drops for agents that cannot use them.

use glam::Vec3;

use super::super::super::agents::TARGET_SAMPLE_DISTANCE;
use super::super::super::test_support::{add_box, move_to};
use super::super::super::NavigationGraph;
use super::{agent, level_scene, spawn, DT};
use rusty_components::NavPathStatus;
use rusty_scene::Scene;

#[test]
fn a_rebake_away_from_the_path_is_not_pending() {
    let mut geo = level_scene(30.0);
    let crate_id = add_box(
        &mut geo,
        Vec3::new(27.0, 0.0, 27.0),
        Vec3::new(28.0, 1.0, 28.0),
    );
    let mut g = NavigationGraph::from_scene(&geo);
    let mut scene = Scene::new();
    let id = spawn(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(2.0, 0.0, 28.0),
    );
    g.tick_nav_agents(&mut scene, DT);
    move_to(&mut geo, crate_id, Vec3::new(27.0, 0.5, 26.0));
    g.sync(&geo);
    let at = scene.world.world_position(id);
    assert_ne!(agent(&scene, id).path_generation, g.bake_generation);
    assert!(!g.path_pending(&agent(&scene, id), at), "its path holds");
    move_to(&mut geo, crate_id, Vec3::new(2.0, 0.5, 15.0));
    g.sync(&geo);
    assert!(
        g.path_pending(&agent(&scene, id), at),
        "a crate on it re-plans"
    );
}

#[test]
fn no_path_to_plan_is_not_pending_and_not_asked_again() {
    let g = NavigationGraph::from_scene(&level_scene(30.0));
    let mut scene = Scene::new();
    let far = Vec3::new(15.0, 0.0, 30.0 + 2.0 * TARGET_SAMPLE_DISTANCE);
    let id = spawn(&mut scene, Vec3::new(2.0, 0.0, 2.0), far);
    g.tick_nav_agents(&mut scene, DT);
    let asked = g.requests.borrow().issued();
    for _ in 0..10 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let a = agent(&scene, id);
    assert_eq!(a.path_status, NavPathStatus::Invalid);
    assert!(
        !g.path_pending(&a, scene.world.world_position(id)),
        "Unity's PathInvalid"
    );
    assert_eq!(g.requests.borrow().issued(), asked, "asked once");
    scene.world.nav_agent_mut(id).expect("agent").target = Vec3::new(2.0, 0.0, 28.0);
    assert!(
        g.path_pending(&agent(&scene, id), scene.world.world_position(id)),
        "a new target"
    );
}

#[test]
fn a_disabled_agents_request_is_dropped() {
    let g = NavigationGraph::from_scene(&level_scene(30.0));
    let mut scene = Scene::new();
    let (from, to) = (Vec3::new(15.0, 0.0, 2.0), Vec3::new(15.0, 0.0, 28.0));
    let dead = spawn(&mut scene, from, to);
    let live = spawn(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(2.0, 0.0, 6.0),
    );
    g.set_pathfinding_iterations_per_frame(8);
    g.tick_nav_agents(&mut scene, DT);
    scene.world.nav_agent_mut(dead).expect("agent").active = false;
    g.tick_nav_agents(&mut scene, DT);
    let a = agent(&scene, dead);
    assert_eq!(a.path_request, 0, "the dead bot's search was dropped");
    assert!(!g.path_pending(&a, from));
    for _ in 0..20 {
        g.tick_nav_agents(&mut scene, DT);
    }
    assert!(
        agent(&scene, live).path_planned,
        "and the one behind it served"
    );
}
