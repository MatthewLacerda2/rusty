//! Agent steering by its numbers, through `tick_nav_agents`: how far one tick from
//! rest moves, that an agent inside its stopping distance holds still and slows, and
//! that a waypoint already reached is passed, not chased.

use glam::Vec3;
use rusty::navigation::NavigationGraph;
use rusty::scene::{NavMeshAgentComponent, Scene};

fn open_graph() -> NavigationGraph {
    NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0)
}

fn agent_toward(target: Vec3) -> NavMeshAgentComponent {
    NavMeshAgentComponent {
        active: true,
        radius: 0.5,
        target,
        speed: 5.0,
        acceleration: 10.0,
        stopping_distance: 0.1,
        velocity: Vec3::ZERO,
        ..Default::default()
    }
}

fn spawn(scene: &mut Scene, pos: Vec3, agent: NavMeshAgentComponent) -> u32 {
    let id = scene.add_entity("a".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_nav_agent(id, Some(agent));
    id
}

/// Acceleration is m/s²: one tick from rest reaches `a·dt` and moves `a·dt²` =
/// 10/3600 ≈ 0.00278, not the `a/dt` step (600 m/s, capped at speed 5) that would
/// move 5/60 ≈ 0.083.
#[test]
fn velocity_step_multiplies_not_divides() {
    let start = Vec3::new(1.0, 0.0, 1.0);
    let target = Vec3::new(10.0, 0.0, 1.0);
    let mut scene = Scene::new();
    let id = spawn(&mut scene, start, agent_toward(target));
    open_graph().tick_nav_agents(&mut scene, 1.0 / 60.0);
    let dx = scene.world.transform(id).unwrap().position.x - 1.0;
    let expected = 10.0 / 3600.0;
    assert!(
        (dx - expected).abs() < 1e-5,
        "dx={dx:.6}, want a·dt²={expected:.6}"
    );
}

/// An agent already inside its stopping distance decelerates in place: it does
/// not steer on toward the destination.
#[test]
fn agent_within_stopping_distance_does_not_advance() {
    let target = Vec3::new(10.0, 0.0, 0.0);
    let start = Vec3::new(9.95, 0.0, 0.0); // dist = 0.05 < stopping_distance = 0.1
    let mut scene = Scene::new();
    let id = spawn(&mut scene, start, agent_toward(target));
    open_graph().tick_nav_agents(&mut scene, 1.0 / 60.0);
    let pos = scene.world.transform(id).unwrap().position;
    assert!(
        (pos.x - 9.95).abs() < 0.002,
        "position must not change inside stopping zone: x={:.4}",
        pos.x
    );
}

/// Inside the stopping distance the velocity shrinks by a share of itself each
/// tick (`velocity -= velocity * factor`): it slows, keeping its sign, and never
/// grows.
#[test]
fn deceleration_reduces_not_amplifies_velocity() {
    let target = Vec3::new(5.0, 0.0, 0.0);
    let start = Vec3::new(4.96, 0.0, 0.0); // dist ≈ 0.04 < stopping_distance = 0.1
    let mut scene = Scene::new();
    let id = scene.add_entity("a".to_string());
    scene.world.transform_mut(id).unwrap().position = start;
    let mut ag = agent_toward(target);
    ag.velocity = Vec3::new(2.0, 0.0, 0.0);
    scene.world.set_nav_agent(id, Some(ag));
    open_graph().tick_nav_agents(&mut scene, 1.0 / 60.0);
    let vel = scene.world.nav_agent(id).unwrap().velocity;
    assert!(
        vel.x > 0.0,
        "velocity must stay positive (not reverse): vx={:.3}",
        vel.x
    );
    assert!(
        vel.x < 2.0,
        "velocity must decrease toward zero: vx={:.3}",
        vel.x
    );
}

/// A waypoint the agent is already within reach of (XZ distance < 0.5), here behind
/// it, is passed: the agent steers to the next one instead of turning back.
#[test]
fn waypoint_behind_agent_is_advanced_past() {
    let start = Vec3::new(5.0, 0.0, 5.0);
    let target = Vec3::new(10.0, 0.0, 5.0);
    let mut scene = Scene::new();
    let id = scene.add_entity("a".to_string());
    scene.world.transform_mut(id).unwrap().position = start;
    let mut ag = agent_toward(target);
    // Pre-load cache: waypoint[0] is 0.2 units behind (< 0.5 reached threshold),
    // waypoint[1] is ahead. Cursor should advance to [1] and steer forward (+x).
    ag.cached_path = vec![
        Vec3::new(4.8, 0.0, 5.0), // behind the agent, within reached distance
        Vec3::new(10.0, 0.0, 5.0),
    ];
    ag.path_cursor = 0;
    ag.planned_target = target;
    ag.path_generation = 0; // matches fresh graph bake_generation = 0
    scene.world.set_nav_agent(id, Some(ag));
    open_graph().tick_nav_agents(&mut scene, 1.0 / 60.0);
    let pos = scene.world.transform(id).unwrap().position;
    // If cursor advanced correctly: steering toward (10, 5) → x increases.
    // If cursor stalled: steering toward (4.8, 5) → x decreases.
    assert!(
        pos.x > 5.0,
        "agent steered forward past the behind-waypoint: x={:.4}",
        pos.x
    );
}
