//! The path queue (#2087): a sliced search finds the synchronous path, a waiting
//! agent follows its old one, and the queue is deterministic and bounded.

use glam::Vec3;

use super::super::agents::TARGET_SAMPLE_DISTANCE;
use super::super::astar::MAX_SEARCH_NODES;
use super::super::test_support::{add_box, add_floor};
use super::super::{reset_path, NavBounds, NavigationGraph, ALL_AREAS};
use crate::components::NavPathStatus;
use crate::scene::{NavMeshAgentComponent, Scene};

const DT: f32 = 1.0 / 60.0;

/// A 30 × 30 floor with a block in the middle, so paths bend around it.
fn level() -> NavigationGraph {
    let mut scene = Scene::new();
    add_floor(&mut scene, -1.0, 31.0, -1.0, 31.0);
    add_box(
        &mut scene,
        Vec3::new(10.0, 0.0, 5.0),
        Vec3::new(20.0, 2.0, 25.0),
    );
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 30.0, 0.0, 30.0));
    scene.nav_settings.grid_spacing = 0.5;
    NavigationGraph::from_scene(&scene)
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

/// The path a synchronous plan from `from` toward `target` gives, start dropped.
fn sync_path(g: &NavigationGraph, from: Vec3, target: Vec3) -> Vec<Vec3> {
    let goal = g
        .sample_position(target, TARGET_SAMPLE_DISTANCE)
        .expect("goal");
    let path = g.calculate_path_masked(from, goal, ALL_AREAS);
    path.corners.into_iter().skip(1).collect()
}

#[test]
fn a_sliced_search_delivers_the_synchronous_path() {
    let g = level();
    let mut scene = Scene::new();
    let (from, to) = (Vec3::new(15.0, 0.0, 2.0), Vec3::new(15.0, 0.0, 28.0));
    let id = spawn(&mut scene, from, to);
    g.set_pathfinding_iterations_per_frame(7);
    let mut ticks = 0;
    while ticks == 0 || agent(&scene, id).path_request != 0 {
        assert!(
            agent(&scene, id).cached_path.is_empty(),
            "nothing to follow yet"
        );
        assert!(g.path_pending(&agent(&scene, id), from));
        g.tick_nav_agents(&mut scene, DT);
        ticks += 1;
    }
    assert!(ticks > 10, "sliced over many ticks: {ticks}");
    assert_eq!(agent(&scene, id).cached_path, sync_path(&g, from, to));
    assert_eq!(agent(&scene, id).path_status, NavPathStatus::Complete);
}

#[test]
fn a_waiting_agent_follows_its_old_path() {
    let g = level();
    let mut scene = Scene::new();
    let from = Vec3::new(2.0, 0.0, 2.0);
    let id = spawn(&mut scene, from, Vec3::new(2.0, 0.0, 28.0));
    g.tick_nav_agents(&mut scene, DT);
    let old = agent(&scene, id).cached_path;
    assert!(!old.is_empty(), "planned in the tick it asked");

    g.set_pathfinding_iterations_per_frame(1);
    scene.world.nav_agent_mut(id).expect("agent").target = Vec3::new(28.0, 0.0, 28.0);
    g.tick_nav_agents(&mut scene, DT);
    let a = agent(&scene, id);
    let at = scene.world.world_position(id);
    assert!(a.path_request != 0 && g.path_pending(&a, at), "waiting");
    assert_eq!(a.cached_path, old, "still on the old path");
    assert!(at.z > from.z, "and walking it: {at}");
}

#[test]
fn reset_path_drops_a_waiting_request() {
    let g = level();
    let mut scene = Scene::new();
    let id = spawn(
        &mut scene,
        Vec3::new(2.0, 0.0, 2.0),
        Vec3::new(28.0, 0.0, 28.0),
    );
    g.set_pathfinding_iterations_per_frame(1);
    g.tick_nav_agents(&mut scene, DT);
    let at = scene.world.world_position(id);
    reset_path(&mut scene.world.nav_agent_mut(id).expect("agent"), at);
    g.tick_nav_agents(&mut scene, DT);
    let a = agent(&scene, id);
    assert_eq!(g.pending_path_requests(), 0, "dropped when it came up");
    assert!(!g.path_pending(&a, at) && a.cached_path.is_empty());
}

#[test]
fn a_rebake_mid_search_restarts_it() {
    let mut g = level();
    let mut scene = Scene::new();
    let (from, to) = (Vec3::new(15.0, 0.0, 2.0), Vec3::new(15.0, 0.0, 28.0));
    let id = spawn(&mut scene, from, to);
    g.set_pathfinding_iterations_per_frame(5);
    g.tick_nav_agents(&mut scene, DT);
    g.bake_generation += 1; // what a rebake does to a search in flight
    for _ in 0..1000 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let a = agent(&scene, id);
    assert_eq!(
        a.path_generation, g.bake_generation,
        "planned on the new bake"
    );
    assert!(
        scene.world.world_position(id).distance(to) < 1.0,
        "and arrived"
    );
}

#[test]
fn queued_agents_are_deterministic() {
    let run = |budget: u32| {
        let g = level();
        let mut scene = Scene::new();
        g.set_pathfinding_iterations_per_frame(budget);
        let ids: Vec<u32> = (0..6)
            .map(|i| {
                let x = 2.0 + 5.0 * i as f32;
                spawn(
                    &mut scene,
                    Vec3::new(x, 0.0, 2.0),
                    Vec3::new(30.0 - x, 0.0, 28.0),
                )
            })
            .collect();
        for _ in 0..240 {
            g.tick_nav_agents(&mut scene, DT);
        }
        let at = |id: u32| scene.world.world_position(id).to_array();
        ids.into_iter().map(at).collect::<Vec<_>>()
    };
    assert_eq!(run(16), run(16), "same scene, same budget, same positions");
}

#[test]
fn an_unreachable_goal_costs_at_most_the_node_cap() {
    // 301² open spans; the goal's area is outside the mask, so A* floods them all.
    let mut g = NavigationGraph::new(0.0, 300.0, 0.0, 300.0, 1.0);
    let goal = g.span_refs().last().expect("spans");
    g.spans[goal.index as usize].area = 5;
    let start = g.span_refs().next().expect("spans");
    let mut pool = super::NodePool::default();
    pool.begin(g.spans.len());
    let mut search = g.begin_search(start, goal, ALL_AREAS & !(1 << 5), &mut pool);
    let mut budget = u32::MAX;
    let (path, complete) = g
        .step_search(&mut search, &mut pool, &mut budget)
        .expect("ends");
    assert!(!complete && path.len() > 1, "a partial path");
    assert_eq!(search.expanded(), MAX_SEARCH_NODES);
    assert!(
        g.spans.len() as u32 > MAX_SEARCH_NODES,
        "the cap, not the mesh, ended it"
    );
}
