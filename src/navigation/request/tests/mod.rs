//! The path queue (#2087): a sliced search finds the synchronous path, a waiting
//! agent follows its old one; `lifecycle` covers rebakes and determinism.

mod anchored;
mod lifecycle;
mod links;
mod pending;

use glam::Vec3;

use super::super::agents::TARGET_SAMPLE_DISTANCE;
use super::super::test_support::{add_box, add_floor};
use super::super::{reset_path, NavBounds, NavigationGraph, ALL_AREAS};
use crate::components::NavPathStatus;
use crate::scene::{NavMeshAgentComponent, Scene};

pub(super) const DT: f32 = 1.0 / 60.0;

/// A 30 × 30 floor with a block in the middle, so paths bend around it.
pub(super) fn level() -> NavigationGraph {
    NavigationGraph::from_scene(&level_scene(30.0))
}

/// [`level`]'s scene, its floor `width` long in x.
pub(super) fn level_scene(width: f32) -> Scene {
    let mut scene = Scene::new();
    add_floor(&mut scene, -1.0, width + 1.0, -1.0, 31.0);
    add_box(
        &mut scene,
        Vec3::new(10.0, 0.0, 5.0),
        Vec3::new(20.0, 2.0, 25.0),
    );
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, width, 0.0, 30.0));
    scene.nav_settings.grid_spacing = 0.5;
    scene
}

pub(super) fn spawn(scene: &mut Scene, at: Vec3, target: Vec3) -> u32 {
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

pub(super) fn agent(scene: &Scene, id: u32) -> NavMeshAgentComponent {
    scene.world.nav_agent(id).expect("agent").clone()
}

/// The path a synchronous plan from `from` toward `target` gives, start dropped.
pub(super) fn sync_path(g: &NavigationGraph, from: Vec3, target: Vec3) -> Vec<Vec3> {
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
    assert_eq!(
        agent(&scene, id).path_cursor,
        0,
        "nothing walked, nothing skipped"
    );
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
