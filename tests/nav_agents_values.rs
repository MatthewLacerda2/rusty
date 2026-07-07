//! Value-asserting agent steering tests for #161 mutation burn-down.
//!
//! `agents.rs` is already at the 300-line cap, so these tests exercise the
//! public `tick_nav_agents` interface to kill arithmetic and comparison mutants
//! that the coverage-only tests miss.

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

/// Kill agents.rs "replace * with /" in velocity blend (line 146).
/// factor = (accel * dt).min(1) ≈ 0.167; with `/` factor is inverted → ×6 speed.
/// After one frame: dx correct ≈ 0.014, wrong ≈ 0.500.
#[test]
fn velocity_blend_multiplies_not_divides() {
    let start = Vec3::new(1.0, 0.0, 1.0);
    let target = Vec3::new(10.0, 0.0, 1.0);
    let mut scene = Scene::new();
    let id = spawn(&mut scene, start, agent_toward(target));
    open_graph().tick_nav_agents(&mut scene, 1.0 / 60.0);
    let dx = scene.world.transform(id).unwrap().position.x - 1.0;
    assert!(dx > 0.005, "agent moved forward: dx={dx:.5}");
    assert!(dx < 0.05, "no over-acceleration (* not /): dx={dx:.5}");
}

/// Kill "replace > with <" in stopping condition (line 109).
/// When dist < stopping_distance the agent must NOT move (decelerate branch).
/// Mutant swaps condition: dist within stopping zone triggers steer instead.
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

/// Kill "replace * with /" in deceleration formula (line 113).
/// Correct: velocity -= velocity * factor (magnitude shrinks).
/// Wrong:   velocity -= velocity / factor (inverted; amplifies and reverses sign).
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

/// Kill "replace sqrt() distance comparison" in waypoint-reached check (line 75).
/// A waypoint BEHIND the agent (XZ distance < 0.5) must be skipped so the agent
/// steers to the NEXT waypoint — not reversed to chase the stale one.
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
