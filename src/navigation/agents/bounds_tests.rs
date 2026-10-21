//! An agent's collider bounds after a tick stand where the agent does (#1153).

use glam::Vec3;

use crate::components::{ColliderComponent, ColliderShape};
use crate::navigation::NavigationGraph;
use crate::scene::{NavMeshAgentComponent, Scene};

#[test]
fn an_agent_under_a_moved_parent_reports_its_bounds_where_it_stands() {
    let mut scene = Scene::new();
    let parent = scene.add_entity("group".into());
    scene.world.transform_mut(parent).unwrap().position = Vec3::new(3.0, 0.0, 4.0);
    let agent = scene.add_entity("agent".into());
    scene.set_parent(agent, Some(parent)).unwrap();
    scene.world.transform_mut(agent).unwrap().position = Vec3::new(2.0, 0.0, 2.0);
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = ColliderShape::Box { size: Vec3::ONE };
    scene.world.set_collider(agent, Some(col));
    scene.world.set_nav_agent(
        agent,
        Some(NavMeshAgentComponent {
            active: true,
            target: Vec3::new(8.0, 0.0, 8.0),
            radius: 0.5,
            speed: 5.0,
            acceleration: 10.0,
            ..Default::default()
        }),
    );

    let graph = NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0);
    for _ in 0..10 {
        graph.tick_nav_agents(&mut scene, 1.0 / 60.0);
    }
    let (min, max) = scene.world.collider_bounds(agent).unwrap();
    let centre = (min + max) * 0.5;
    let stands = scene.world.world_position(agent);
    assert!(centre.abs_diff_eq(stands, 1e-4), "{centre} vs {stands}");
    assert!(stands.x > 3.0 + 2.0, "it walked: {stands}");
}
