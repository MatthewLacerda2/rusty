//! What a delivered path holds (#2087): nothing installed while the agent drops
//! off a ledge, and no turning back to a corner it walked past while it waited.

use glam::Vec3;

use super::super::super::test_support::{add_box, add_floor};
use super::super::super::{NavBounds, NavigationGraph};
use super::{agent, level, spawn, DT};
use rusty_scene::Scene;

/// A 3 m platform over x 0..8 on a 60 m floor, drops generated off its edge.
fn ledge() -> NavigationGraph {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 1.0;
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 60.0, 0.0, 10.0));
    scene.nav_settings.drop_height = 4.0;
    add_floor(&mut scene, -1.0, 61.0, -1.0, 11.0);
    add_box(
        &mut scene,
        Vec3::new(-1.0, 0.0, -1.0),
        Vec3::new(8.0, 3.0, 11.0),
    );
    NavigationGraph::from_scene(&scene)
}

#[test]
fn no_path_is_installed_while_the_agent_crosses_a_link() {
    let g = ledge();
    let mut scene = Scene::new();
    let id = spawn(
        &mut scene,
        Vec3::new(2.0, 3.0, 5.0),
        Vec3::new(55.0, 0.0, 5.0),
    );
    g.tick_nav_agents(&mut scene, DT);
    assert!(
        !agent(&scene, id).path_links.is_empty(),
        "the path drops off the ledge"
    );
    // A new target, answered one expansion a tick: still searching at the ledge.
    g.set_pathfinding_iterations_per_frame(1);
    scene.world.nav_agent_mut(id).expect("agent").target = Vec3::new(55.0, 0.0, 6.0);
    let mut ticks = 0;
    while agent(&scene, id).off_mesh_link.is_none() {
        assert!(ticks < 600, "reaches the ledge");
        g.tick_nav_agents(&mut scene, DT);
        ticks += 1;
    }
    assert!(
        agent(&scene, id).path_request != 0,
        "the request was still waiting"
    );
    let crossing = agent(&scene, id).cached_path;
    while agent(&scene, id).off_mesh_link.is_some() {
        g.tick_nav_agents(&mut scene, DT);
        let a = agent(&scene, id);
        assert_eq!(a.cached_path, crossing, "no path lands mid-drop");
        assert_eq!(a.path_request, 0, "the request was given up");
    }
    g.set_pathfinding_iterations_per_frame(4096);
    for _ in 0..1200 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let at = scene.world.world_position(id);
    assert!(
        at.distance(Vec3::new(55.0, 0.0, 6.0)) < 1.0,
        "re-asked on landing: {at}"
    );
}

#[test]
fn a_delivered_path_skips_the_corner_walked_past() {
    let g = level();
    let mut scene = Scene::new();
    // First west along z = 3, under the block (x 10..20, z 5..25).
    let id = spawn(
        &mut scene,
        Vec3::new(12.0, 0.0, 3.0),
        Vec3::new(2.0, 0.0, 3.0),
    );
    g.tick_nav_agents(&mut scene, DT);
    // Then north past the block's corner, answered while it walks on west.
    g.set_pathfinding_iterations_per_frame(1);
    let goal = Vec3::new(5.0, 0.0, 28.0);
    scene.world.nav_agent_mut(id).expect("agent").target = goal;
    g.tick_nav_agents(&mut scene, DT);
    while agent(&scene, id).path_request != 0 {
        g.tick_nav_agents(&mut scene, DT);
    }
    let a = agent(&scene, id);
    let at = scene.world.world_position(id);
    assert!(a.cached_path[0].x >= 9.0, "the path turns at the corner");
    assert!(at.x < 8.0, "which it walked past while it waited: {at}");
    assert!(a.path_cursor >= 1, "which the agent does not walk back to");
    assert!(a.cached_path[a.path_cursor].z > 4.0, "it heads north");
}
