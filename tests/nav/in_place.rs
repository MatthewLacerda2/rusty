//! The navigation tick edits each agent where it sits: a walking agent's entity
//! never changes hecs archetype, so it keeps every other component and its place
//! in the insertion order, and no column is detached and attached again a tick.

use glam::Vec3;
use rusty::ecs::walks;
use rusty::navigation::NavigationGraph;
use rusty::scene::{NavMeshAgentComponent, Scene};

const DT: f32 = 1.0 / 60.0;

/// An agent with a collider at x = 1 on an open 40 m floor, heading for x = 38.
fn walker() -> (Scene, u32, NavigationGraph) {
    let mut scene = Scene::new();
    scene.add_entity("before".to_string());
    let id = scene.add_entity("walker".to_string());
    scene.add_entity("after".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(1.0, 0.0, 1.0);
    scene
        .world
        .set_collider(id, Some(rusty::edit::authoring::default_collider()));
    let agent = NavMeshAgentComponent {
        active: true,
        radius: 0.5,
        target: Vec3::new(38.0, 0.0, 1.0),
        speed: 3.5,
        acceleration: 8.0,
        stopping_distance: 0.1,
        ..Default::default()
    };
    scene.world.set_nav_agent(id, Some(agent));
    (scene, id, NavigationGraph::new(0.0, 40.0, 0.0, 2.0, 1.0))
}

#[test]
fn a_walking_agent_keeps_its_archetype_across_ticks() {
    let (mut scene, id, graph) = walker();
    let seq = scene.world.seq(id);
    let ids = scene.world.ids_with_nav_agent();
    let before = walks::moves();
    for _ in 0..30 {
        graph.tick_nav_agents(&mut scene, DT);
    }
    assert_eq!(walks::moves(), before, "a tick moved an entity's archetype");
    assert!(
        scene.world.transform(id).unwrap().position.x > 1.5,
        "it walked"
    );
    assert!(scene.world.nav_agent(id).unwrap().velocity.x > 0.0);
    assert!(scene.world.has_collider(id), "its other components stay");
    assert_eq!(scene.world.seq(id), seq);
    assert_eq!(scene.world.ids_with_nav_agent(), ids);
}

/// Root motion's slide (`NavigationGraph::move_agent`) edits the agent in place
/// too, and gives it back its own velocity.
#[test]
fn a_root_motion_step_keeps_the_archetype() {
    let (mut scene, id, graph) = walker();
    scene.world.nav_agent_mut(id).unwrap().velocity = Vec3::new(0.0, 0.0, 2.0);
    let before = walks::moves();
    graph.move_agent(&mut scene, id, Vec3::new(0.5, 0.0, 0.0));
    assert_eq!(walks::moves(), before);
    let x = scene.world.transform(id).unwrap().position.x;
    assert!((x - 1.5).abs() < 1e-5, "moved by the motion: {x}");
    let v = scene.world.nav_agent(id).unwrap().velocity;
    assert_eq!(v, Vec3::new(0.0, 0.0, 2.0));
}

/// The counter itself: a detach and a re-attach are two moves, an overwrite none.
#[test]
fn detach_and_attach_count_as_moves() {
    let (mut scene, id, _) = walker();
    let before = walks::moves();
    let agent = scene.world.take_nav_agent(id).unwrap();
    scene.world.set_nav_agent(id, Some(agent.clone()));
    assert_eq!(walks::moves(), before + 2);
    scene.world.set_nav_agent(id, Some(agent));
    assert_eq!(walks::moves(), before + 2, "an overwrite moves nothing");
}
