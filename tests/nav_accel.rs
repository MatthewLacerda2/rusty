//! `NavMeshAgent.acceleration` is a constant rate in m/s² and the agent brakes
//! into its stopping distance at that same rate (#742, Unity's `autoBraking`).

use glam::Vec3;
use rusty::navigation::NavigationGraph;
use rusty::scene::{NavMeshAgentComponent, Scene};

const DT: f32 = 1.0 / 60.0;

fn agent(speed: f32, acceleration: f32, stopping_distance: f32) -> NavMeshAgentComponent {
    NavMeshAgentComponent {
        active: true,
        radius: 0.5,
        target: Vec3::new(38.0, 0.0, 1.0),
        speed,
        acceleration,
        stopping_distance,
        ..Default::default()
    }
}

/// An agent at x = 1 on an open 40 m floor, heading for x = 38.
fn spawn(a: NavMeshAgentComponent) -> (Scene, u32, NavigationGraph) {
    let mut scene = Scene::new();
    let id = scene.add_entity("a".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(1.0, 0.0, 1.0);
    scene.world.set_nav_agent(id, Some(a));
    (scene, id, NavigationGraph::new(0.0, 40.0, 0.0, 2.0, 1.0))
}

fn speed(scene: &Scene, id: u32) -> f32 {
    scene.world.nav_agent(id).unwrap().velocity.length()
}

/// Ticks from rest until the agent reaches full speed.
fn ticks_to_full_speed(speed_limit: f32, acceleration: f32) -> u32 {
    let (mut scene, id, graph) = spawn(agent(speed_limit, acceleration, 0.1));
    (1..=600)
        .find(|_| {
            graph.tick_nav_agents(&mut scene, DT);
            speed(&scene, id) >= speed_limit - 1e-4
        })
        .expect("never reached full speed")
}

/// Time to full speed is `speed / acceleration`, and the agent lands on `speed`
/// exactly, where the old lerp took the same time for any speed and only
/// approached it.
#[test]
fn time_to_full_speed_is_speed_over_acceleration() {
    // 3.5 / 8 = 0.4375 s = 26.25 ticks; 7 / 8 = 52.5 ticks.
    assert_eq!(ticks_to_full_speed(3.5, 8.0), 27);
    assert_eq!(ticks_to_full_speed(7.0, 8.0), 53);
    let (mut scene, id, graph) = spawn(agent(3.5, 8.0, 0.1));
    for _ in 0..60 {
        graph.tick_nav_agents(&mut scene, DT);
    }
    let v = speed(&scene, id);
    assert!((v - 3.5).abs() < 1e-5, "lands on speed, no asymptote: {v}");
}

/// Zero acceleration never speeds up (Unity's meaning).
#[test]
fn zero_acceleration_never_moves() {
    let (mut scene, id, graph) = spawn(agent(3.5, 0.0, 0.1));
    for _ in 0..60 {
        graph.tick_nav_agents(&mut scene, DT);
    }
    assert_eq!(scene.world.transform(id).unwrap().position.x, 1.0);
}

/// The agent brakes before arriving and comes to rest at its stopping distance:
/// it starts slowing about `v²/(2a)` ≈ 0.77 m out, reports zero velocity once
/// still, and neither stops short nor overshoots by more than 2 mm.
#[test]
fn brakes_into_the_stopping_distance() {
    let (mut scene, id, graph) = spawn(agent(3.5, 8.0, 1.5));
    let mut braking_from = None;
    let mut top = 0.0f32;
    for _ in 0..900 {
        graph.tick_nav_agents(&mut scene, DT);
        let (x, v) = (
            scene.world.transform(id).unwrap().position.x,
            speed(&scene, id),
        );
        if v < top - 0.01 && braking_from.is_none() {
            braking_from = Some(38.0 - x);
        }
        top = top.max(v);
    }
    let x = scene.world.transform(id).unwrap().position.x;
    assert_eq!(speed(&scene, id), 0.0, "at rest, reported velocity is zero");
    assert!(
        (38.0 - x - 1.5).abs() < 0.002,
        "rests at the stopping distance: x={x}"
    );
    let from = braking_from.expect("never braked") - 1.5;
    assert!(
        (from - 0.77).abs() < 0.1,
        "braked {from:.3} m before the stop"
    );
}

/// Inside the stopping distance an agent still moving keeps moving while it
/// brakes, instead of freezing in place with a stale velocity.
#[test]
fn braking_inside_the_stopping_distance_still_moves() {
    let mut a = agent(3.5, 8.0, 1.5);
    a.velocity = Vec3::new(2.0, 0.0, 0.0);
    a.target = Vec3::new(2.0, 0.0, 1.0);
    let (mut scene, id, graph) = spawn(a);
    graph.tick_nav_agents(&mut scene, DT);
    let x = scene.world.transform(id).unwrap().position.x;
    let v = 2.0 - 8.0 * DT;
    assert!(
        (x - 1.0 - v * DT).abs() < 1e-5,
        "moved by the braked velocity: x={x}"
    );
    assert!((speed(&scene, id) - v).abs() < 1e-5);
}

/// An agent that has arrived stands its ground: it is not steering, so it does
/// not dodge a passer-by (only steering agents solve avoidance) even while it
/// still holds the path it arrived on.
#[test]
fn an_arrived_agent_does_not_dodge() {
    let home = Vec3::new(10.0, 0.0, 10.0);
    let mut arrived = agent(3.5, 8.0, 0.2);
    arrived.target = home;
    arrived.planned_target = home;
    arrived.cached_path = vec![home];
    let mut scene = Scene::new();
    let a = scene.add_entity("arrived".to_string());
    scene.world.transform_mut(a).unwrap().position = home;
    scene.world.set_nav_agent(a, Some(arrived));
    let mut walker = agent(3.5, 8.0, 0.2);
    walker.target = Vec3::new(18.0, 0.0, 10.1);
    let b = scene.add_entity("walker".to_string());
    scene.world.transform_mut(b).unwrap().position = Vec3::new(2.0, 0.0, 10.1);
    scene.world.set_nav_agent(b, Some(walker));
    let graph = NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0);
    for _ in 0..300 {
        graph.tick_nav_agents(&mut scene, DT);
    }
    assert_eq!(scene.world.transform(a).unwrap().position, home, "dodged");
}
