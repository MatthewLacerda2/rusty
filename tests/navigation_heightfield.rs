//! Height-field navigation (#130): A* climbs ramps/stairs with correct `y`, tall
//! steps block, agents ride the surface, and replay is byte-identical. Exercises
//! only the public `NavigationGraph` surface.

use glam::Vec3;
use rusty::navigation::NavigationGraph;
use rusty::scene::{ColliderComponent, ColliderShape, NavMeshAgentComponent, Scene};

fn add_box(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) {
    let id = scene.add_entity(name.to_string());
    let mut e = scene.get_entity_mut(id).expect("entity exists");
    e.is_static = true;
    e.collider = Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size: max - min },
        is_trigger: false,
        aabb_min: min,
        aabb_max: max,
    });
}

/// A ramp rising 0.5/cell along +x (cells x=3..=7), spanning z=2..6, ground else.
fn ramp_graph() -> NavigationGraph {
    let mut scene = Scene::new();
    for (i, gx) in (3..=7).enumerate() {
        let y = 0.5 * i as f32;
        let fx = gx as f32;
        add_box(
            &mut scene,
            &format!("ramp{i}"),
            Vec3::new(fx - 0.5, 0.0, 2.0),
            Vec3::new(fx + 0.5, y, 6.0),
        );
    }
    let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    graph.bake(&scene);
    graph
}

#[test]
fn path_climbs_ramp_with_rising_height() {
    let graph = ramp_graph();
    let start = graph.grid_to_world(1, 4);
    let target = graph.grid_to_world(7, 4);
    let (sx, sz) = graph.world_to_grid(start);
    let (tx, tz) = graph.world_to_grid(target);
    let path = graph.find_path(sx, sz, tx, tz).expect("a path up the ramp");

    let heights: Vec<f32> = path
        .iter()
        .map(|&(gx, gz)| graph.height_at(gx, gz))
        .collect();
    for w in heights.windows(2) {
        assert!(w[1] >= w[0], "ramp climb must not descend: {heights:?}");
    }
    assert_eq!(*heights.last().unwrap(), 2.0, "reaches the elevated top");
}

#[test]
fn next_step_carries_surface_height() {
    let graph = ramp_graph();
    let start = graph.grid_to_world(3, 4); // y = 0.0
    let target = graph.grid_to_world(7, 4); // y = 2.0
    let step = graph.get_next_path_step(start, target);
    assert!(step.y > 0.0, "next step climbs in y, got {}", step.y);
}

#[test]
fn tall_step_blocks_path() {
    // Wall (top y=2 > max_step 0.5) spanning all z cuts x=4..=5: no crossing.
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        "wall",
        Vec3::new(4.0, 0.0, 0.0),
        Vec3::new(5.0, 2.0, 10.0),
    );
    let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
    graph.bake(&scene);
    if let Some(p) = graph.find_path(1, 5, 9, 5) {
        let reached = p.last().copied().unwrap_or((0, 0));
        assert!(reached.0 < 4, "must not cross the impassable wall: {p:?}");
    }
}

#[test]
fn find_path_is_deterministic() {
    let graph = ramp_graph();
    assert_eq!(
        graph.find_path(1, 4, 7, 4),
        graph.find_path(1, 4, 7, 4),
        "A* path must be byte-identical across runs"
    );
}

#[test]
fn agent_rides_ramp_to_elevated_target() {
    let run = || {
        let graph = ramp_graph();
        let mut scene = Scene::new();
        let id = scene.add_entity("agent".to_string());
        {
            let mut e = scene.get_entity_mut(id).expect("entity exists");
            e.transform.position = graph.grid_to_world(1, 4);
            e.nav_agent = Some(NavMeshAgentComponent {
                active: true,
                radius: 0.5,
                target: graph.grid_to_world(7, 4), // y = 2.0
                speed: 4.0,
                acceleration: 10.0,
                stopping_distance: 0.5,
                velocity: Vec3::ZERO,
                ..Default::default()
            });
        }
        let dt = 1.0 / 60.0;
        for _ in 0..900 {
            graph.tick_nav_agents(&mut scene, dt);
        }
        let guard = scene.get_entity(id).expect("entity");
        guard.transform.position
    };

    let pos = run();
    // The agent climbed: it ends well above the ground it started on.
    assert!(
        pos.y > 1.0,
        "agent should ride up the ramp, got y={}",
        pos.y
    );
    // And replay is byte-identical.
    assert_eq!(
        pos.to_array(),
        run().to_array(),
        "agent pathing is deterministic"
    );
}
