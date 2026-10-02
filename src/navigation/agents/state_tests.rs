//! Agent path state (#458): what scripts read from an agent's path, `Warp` and
//! `ResetPath`, and stopping at the end of a partial path.

use glam::Vec3;

use super::super::test_support::{add_box, add_floor, bake_pinned};
use super::super::{remaining_distance, reset_path, NavigationGraph};
use super::state::remaining_corners;
use crate::components::NavPathStatus;
use crate::scene::{NavMeshAgentComponent, Scene};

const DT: f32 = 1.0 / 60.0;

/// A floor over the grid, with a full-length wall at x = 5 when `walled`.
fn level(walled: bool) -> (Scene, NavigationGraph) {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    if walled {
        add_box(
            &mut scene,
            Vec3::new(4.75, 0.0, -5.0),
            Vec3::new(5.25, 3.0, 15.0),
        );
    }
    let g = bake_pinned(&mut scene);
    (scene, g)
}

fn spawn(scene: &mut Scene, at: Vec3, target: Vec3) -> u32 {
    let id = scene.add_entity("agent".to_string());
    scene.world.transform_mut(id).expect("transform").position = at;
    let agent = NavMeshAgentComponent {
        active: true,
        radius: 0.5,
        target,
        speed: 4.0,
        acceleration: 20.0,
        stopping_distance: 0.1,
        ..Default::default()
    };
    scene.world.set_nav_agent(id, Some(agent));
    id
}

fn agent(scene: &Scene, id: u32) -> NavMeshAgentComponent {
    scene.world.nav_agent(id).expect("agent").clone()
}

#[test]
fn straight_path_state_reads_back() {
    let (mut scene, g) = level(false);
    let id = spawn(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(8.0, 0.0, 2.0),
    );
    let a = agent(&scene, id);
    assert!(
        remaining_distance(&a, Vec3::ZERO).is_infinite(),
        "no path yet"
    );
    g.tick_nav_agents(&mut scene, DT);
    let a = agent(&scene, id);
    let at = scene.world.transform(id).expect("transform").position;
    assert_eq!(a.path_status, NavPathStatus::Complete);
    assert_eq!(
        a.cached_path,
        vec![Vec3::new(8.0, 0.0, 2.0)],
        "one straight leg"
    );
    assert_eq!(remaining_corners(&a, at).len(), 2);
    assert!((remaining_distance(&a, at) - (8.0 - at.x)).abs() < 1e-4);
}

#[test]
fn agent_stops_at_the_end_of_a_partial_path() {
    let (mut scene, g) = level(true);
    let id = spawn(
        &mut scene,
        Vec3::new(1.0, 0.0, 5.0),
        Vec3::new(8.0, 0.0, 5.0),
    );
    for _ in 0..300 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let a = agent(&scene, id);
    assert_eq!(a.path_status, NavPathStatus::Partial);
    let at = scene.world.transform(id).expect("transform").position;
    assert!(at.distance(a.destination()) <= 0.1, "parked at {at}");
    assert_eq!(
        a.velocity,
        Vec3::ZERO,
        "stopped, not pressing into the wall"
    );
}

#[test]
fn warp_moves_the_agent_and_plans_once() {
    let (mut scene, g) = level(false);
    let id = spawn(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(8.0, 0.0, 8.0),
    );
    g.tick_nav_agents(&mut scene, DT);
    let mut a = agent(&scene, id);
    let landed = g
        .warp_agent(&mut a, Vec3::new(2.0, 0.7, 8.0))
        .expect("on the floor");
    assert_eq!(landed, Vec3::new(2.0, 0.0, 8.0), "snapped onto the floor");
    assert!(a.cached_path.is_empty() && a.velocity == Vec3::ZERO);
    assert!(
        g.warp_agent(&mut a, Vec3::new(2.0, 9.0, 8.0)).is_none(),
        "too far"
    );
    scene.world.set_nav_agent(id, Some(a));
    scene.world.transform_mut(id).expect("transform").position = landed;
    for _ in 0..10 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let a = agent(&scene, id);
    assert_eq!(
        a.frames_since_replan, 10,
        "one plan after the warp, then reused"
    );
    assert_eq!(
        a.cached_path,
        vec![Vec3::new(8.0, 0.0, 8.0)],
        "planned from the warp"
    );
}

#[test]
fn reset_path_stops_the_agent_where_it_stands() {
    let (mut scene, g) = level(false);
    let id = spawn(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(8.0, 0.0, 8.0),
    );
    g.tick_nav_agents(&mut scene, DT);
    let at = scene.world.transform(id).expect("transform").position;
    let mut a = agent(&scene, id);
    reset_path(&mut a, at);
    assert_eq!(a.target, at);
    assert!(a.cached_path.is_empty());
    scene.world.set_nav_agent(id, Some(a));
    for _ in 0..120 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let a = agent(&scene, id);
    assert!(
        a.cached_path.is_empty(),
        "nothing planned without a destination"
    );
    assert_eq!(a.velocity, Vec3::ZERO);
}
