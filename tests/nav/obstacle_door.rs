//! A carving `NavMeshObstacle` (#456) closing a doorway mid-walk: the agent heading
//! for it re-plans through the other doorway the same tick the door carves, and
//! arrives; a replay is bit-identical. A non-carving obstacle is steered around.

use glam::Vec3;
use rusty::components::NavMeshObstacleComponent;
use rusty::navigation::{tick_obstacles, NavBounds, NavigationGraph};
use rusty::scene::{NavMeshAgentComponent, Scene};

use crate::navigation_layered::add_box;

const DT: f32 = 1.0 / 60.0;
const GOAL: Vec3 = Vec3::new(25.0, 0.0, 6.0);

/// A 30 × 20 floor split at x = 15, doorway A over z 4..8, doorway B over z 14..18;
/// an agent west of A bound east, and a door obstacle (not carving yet) in A.
fn level() -> (Scene, NavigationGraph, u32, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 30.0, 0.0, 20.0));
    add_box(
        &mut scene,
        Vec3::new(-1.0, -0.2, -1.0),
        Vec3::new(31.0, 0.0, 21.0),
    );
    for (z0, z1) in [(-1.0, 4.0), (8.0, 14.0), (18.0, 21.0)] {
        add_box(
            &mut scene,
            Vec3::new(14.8, 0.0, z0),
            Vec3::new(15.2, 3.0, z1),
        );
    }
    let door = scene.add_entity("door".to_string());
    scene.world.transform_mut(door).unwrap().position = Vec3::new(15.0, 1.0, 6.0);
    let o = NavMeshObstacleComponent {
        size: Vec3::new(0.4, 2.0, 4.0),
        ..Default::default()
    };
    scene.world.set_nav_obstacle(door, Some(o));
    let agent = scene.add_entity("agent".to_string());
    scene.world.transform_mut(agent).unwrap().position = Vec3::new(3.0, 0.0, 6.0);
    let a = NavMeshAgentComponent {
        active: true,
        radius: 0.4,
        target: GOAL,
        speed: 4.0,
        acceleration: 10.0,
        stopping_distance: 0.2,
        ..Default::default()
    };
    scene.world.set_nav_agent(agent, Some(a));
    let nav = NavigationGraph::from_scene(&scene);
    (scene, nav, door, agent)
}

/// The play-mode order: obstacle bookkeeping, incremental rebake, agent steering.
fn tick(scene: &mut Scene, nav: &mut NavigationGraph) {
    tick_obstacles(scene, DT);
    nav.sync(scene);
    nav.tick_nav_agents(scene, DT);
}

/// Close door A after one second; returns every position the agent passed.
fn run() -> Vec<Vec3> {
    let (mut scene, mut nav, door, agent) = level();
    let mut trail = Vec::new();
    for frame in 0..900 {
        if frame == 60 {
            let mut o = scene.world.nav_obstacle_mut(door).unwrap();
            o.carve = true; // Closes A; carves at once: it has not moved.
        }
        tick(&mut scene, &mut nav);
        trail.push(scene.world.transform(agent).unwrap().position);
    }
    trail
}

#[test]
fn a_door_closing_mid_walk_reroutes_the_agent_through_the_other_doorway() {
    let trail = run();
    let before = trail[59];
    assert!(
        before.x > 5.0 && (before.z - 6.0).abs() < 0.5,
        "headed for A: {before}"
    );
    let crossing = trail.windows(2).find(|w| w[0].x < 15.0 && w[1].x >= 15.0);
    let crossing = crossing.expect("the agent crosses the wall")[1];
    assert!(
        crossing.z > 13.5,
        "went through doorway B, crossed at {crossing}"
    );
    let end = *trail.last().unwrap();
    assert!(end.distance(GOAL) < 0.5, "arrived: {end}");
    assert_eq!(trail, run(), "replay is bit-identical");
}

#[test]
fn a_non_carving_obstacle_in_the_way_is_steered_around() {
    let (mut scene, mut nav, door, agent) = level();
    // Park the (non-carving) door just off the agent's line, in the open room. Dead
    // ahead, ORCA has no side to prefer and stalls, as Unity's avoidance does: a
    // stationary obstacle should carve.
    scene.world.transform_mut(door).unwrap().position = Vec3::new(9.0, 1.0, 6.4);
    scene.world.nav_obstacle_mut(door).unwrap().size = Vec3::ONE; // a crate
    let mut closest = f32::MAX;
    for _ in 0..600 {
        tick(&mut scene, &mut nav);
        let p = scene.world.transform(agent).unwrap().position;
        closest = closest.min(Vec3::new(p.x - 9.0, 0.0, p.z - 6.4).length());
    }
    assert!(
        closest > 0.4,
        "kept clear of the obstacle, closest {closest}"
    );
    let end = scene.world.transform(agent).unwrap().position;
    assert!(end.distance(GOAL) < 0.5, "arrived: {end}");
}
