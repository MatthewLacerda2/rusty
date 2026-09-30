//! Local avoidance through a choke (#463): eight agents funnel through a door two
//! agents wide without overlapping, all arrive, and a replay is bit-identical.

use glam::Vec3;
use rusty::navigation::{NavBounds, NavigationGraph};
use rusty::scene::{ColliderComponent, ColliderShape, NavMeshAgentComponent, Scene};

const DT: f32 = 1.0 / 60.0;
const RADIUS: f32 = 0.5;

fn add_wall(scene: &mut Scene, min: Vec3, max: Vec3) {
    let id = scene.add_entity("wall".to_string());
    scene.world.set_static(id, true);
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size: max - min },
            is_trigger: false,
            aabb_min: min,
            aabb_max: max,
        }),
    );
}

/// A wall along x = 10 with a door over cells z = 10..=11 (world z ∈ [9.5, 11.5),
/// two agent diameters), eight agents west of it bound for spread-out goals east.
fn door_scene() -> (Scene, NavigationGraph, Vec<(u32, Vec3)>) {
    let mut scene = Scene::new();
    add_wall(
        &mut scene,
        Vec3::new(9.5, 0.0, 0.0),
        Vec3::new(10.5, 3.0, 9.4),
    );
    add_wall(
        &mut scene,
        Vec3::new(9.5, 0.0, 11.6),
        Vec3::new(10.5, 3.0, 20.0),
    );
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 20.0));
    scene.nav_settings.agent_radius = 0.0; // Keep the door two cells wide.
    let mut graph = NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0);
    graph.bake(&scene);

    let mut agents = Vec::new();
    for i in 0..8 {
        let (col, row) = ((i / 4) as f32, (i % 4) as f32);
        let start = Vec3::new(4.0 + 1.5 * col, 0.0, 8.0 + 1.5 * row);
        let goal = Vec3::new(14.0 + 3.0 * col, 0.0, 5.5 + 3.0 * row);
        let id = scene.add_entity(format!("agent{i}"));
        scene.world.transform_mut(id).unwrap().position = start;
        let agent = NavMeshAgentComponent {
            active: true,
            radius: RADIUS,
            target: goal,
            speed: 3.0,
            acceleration: 8.0,
            stopping_distance: 0.2,
            ..Default::default()
        };
        scene.world.set_nav_agent(id, Some(agent));
        agents.push((id, goal));
    }
    (scene, graph, agents)
}

/// Run the door scenario; returns the smallest centre gap seen between any two
/// agents and every agent's final position.
fn run_door() -> (f32, Vec<Vec3>) {
    let (mut scene, graph, agents) = door_scene();
    let mut min_gap = f32::MAX;
    for _ in 0..1800 {
        graph.tick_nav_agents(&mut scene, DT);
        let ps: Vec<Vec3> = agents
            .iter()
            .map(|(id, _)| scene.world.transform(*id).unwrap().position)
            .collect();
        for (i, a) in ps.iter().enumerate() {
            for b in &ps[i + 1..] {
                min_gap = min_gap.min(a.distance(*b));
            }
        }
    }
    let ends = agents
        .iter()
        .map(|(id, _)| scene.world.transform(*id).unwrap().position)
        .collect();
    (min_gap, ends)
}

#[test]
fn eight_agents_funnel_through_a_two_wide_door_and_all_arrive() {
    let (min_gap, ends) = run_door();
    let (_, _, agents) = door_scene();
    assert!(
        min_gap >= 2.0 * RADIUS * 0.9,
        "agents overlapped: min gap {min_gap}"
    );
    for ((_, goal), end) in agents.iter().zip(&ends) {
        let d = Vec3::new(end.x - goal.x, 0.0, end.z - goal.z).length();
        assert!(d < 0.6, "agent bound for {goal:?} ended at {end:?}");
    }
}

#[test]
fn door_replay_is_bit_identical() {
    let bits = |ps: Vec<Vec3>| -> Vec<[u32; 3]> {
        ps.iter()
            .map(|p| [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()])
            .collect()
    };
    let (gap_a, a) = run_door();
    let (gap_b, b) = run_door();
    assert_eq!(gap_a.to_bits(), gap_b.to_bits());
    assert_eq!(bits(a), bits(b));
}
