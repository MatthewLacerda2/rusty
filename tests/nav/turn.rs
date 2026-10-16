//! A `NavMeshAgent` turns itself to face where it steers, at a limited rate (#744).

use std::f32::consts::FRAC_PI_2;

use glam::{EulerRot, Quat, Vec3};
use rusty::navigation::NavigationGraph;
use rusty::scene::{NavMeshAgentComponent, Scene};

const DT: f32 = 1.0 / 60.0;

fn agent(target_x: f32) -> NavMeshAgentComponent {
    NavMeshAgentComponent {
        active: true,
        radius: 0.5,
        target: Vec3::new(target_x, 0.0, 1.0),
        speed: 3.5,
        acceleration: 8.0,
        stopping_distance: 0.1,
        ..Default::default()
    }
}

fn spawn(scene: &mut Scene, x: f32, yaw: f32, a: NavMeshAgentComponent) -> u32 {
    let id = scene.add_entity("a".to_string());
    {
        let mut t = scene.world.transform_mut(id).unwrap();
        t.position = Vec3::new(x, 0.0, 1.0);
        t.rotation = Quat::from_rotation_y(yaw);
    }
    scene.world.set_nav_agent(id, Some(a));
    id
}

fn yaw_deg(scene: &Scene, id: u32) -> f32 {
    let q = scene.world.transform(id).unwrap().rotation;
    q.to_euler(EulerRot::YXZ).0.to_degrees()
}

/// An open 40 m strip along +X.
fn graph() -> NavigationGraph {
    NavigationGraph::new(0.0, 40.0, 0.0, 2.0, 1.0)
}

/// Facing +Z and sent along +X, it eases round to +90° over about a second instead
/// of snapping, and stays there.
#[test]
fn turns_to_its_heading_at_a_limited_rate() {
    let (mut scene, graph) = (Scene::new(), graph());
    let id = spawn(&mut scene, 1.0, 0.0, agent(38.0));
    graph.tick_nav_agents(&mut scene, DT);
    let first = yaw_deg(&scene, id);
    assert!(first > 0.0 && first < 1.0, "eases in, no snap: {first}");
    for _ in 0..90 {
        graph.tick_nav_agents(&mut scene, DT);
    }
    assert!((yaw_deg(&scene, id) - 90.0).abs() < 1e-3);
}

/// With `update_rotation` off the engine never touches the rotation.
#[test]
fn update_rotation_off_leaves_rotation_to_scripts() {
    let (mut scene, graph) = (Scene::new(), graph());
    let id = spawn(
        &mut scene,
        1.0,
        0.0,
        NavMeshAgentComponent {
            update_rotation: false,
            ..agent(38.0)
        },
    );
    for _ in 0..60 {
        graph.tick_nav_agents(&mut scene, DT);
    }
    assert!(
        scene.world.transform(id).unwrap().position.x > 1.5,
        "it moved"
    );
    assert_eq!(yaw_deg(&scene, id), 0.0);
}

/// Two agents meeting head-on dodge each other: their velocity swings well off
/// the path, but each keeps facing along it (the steering direction, toward the
/// next corner, not the post-avoidance velocity), so the facing doesn't flick.
#[test]
fn faces_the_steering_direction_not_the_dodge() {
    let (mut scene, graph) = (Scene::new(), graph());
    let a = spawn(&mut scene, 4.0, FRAC_PI_2, agent(36.0));
    spawn(&mut scene, 36.0, -FRAC_PI_2, agent(4.0));
    let (mut dodge, mut lag) = (0.0f32, 0.0f32);
    for _ in 0..300 {
        graph.tick_nav_agents(&mut scene, DT);
        let v = scene.world.nav_agent(a).unwrap().velocity;
        if v.length() < 0.5 {
            continue;
        }
        // Open floor: the path is one straight leg, so it steers at its target.
        let to = Vec3::new(36.0, 0.0, 1.0) - scene.world.transform(a).unwrap().position;
        let steer = to.x.atan2(to.z).to_degrees();
        dodge = dodge.max((v.x.atan2(v.z).to_degrees() - steer).abs());
        lag = lag.max((yaw_deg(&scene, a) - steer).abs());
    }
    assert!(dodge > 5.0, "they did dodge: {dodge}°");
    assert!(
        lag < 0.5,
        "facing strayed {lag}° from the steering direction"
    );
}

/// A scene saved before #744 has no turning fields: they load as Unity's defaults
/// (turning on, 120°/s) and the 720°/s² ramp.
#[test]
fn older_scenes_load_the_turning_defaults() {
    let mut json = serde_json::to_value(NavMeshAgentComponent {
        update_rotation: false,
        angular_speed: 1.0,
        angular_acceleration: 1.0,
        ..agent(0.0)
    })
    .unwrap();
    for key in ["update_rotation", "angular_speed", "angular_acceleration"] {
        json.as_object_mut().unwrap().remove(key);
    }
    let a: NavMeshAgentComponent = serde_json::from_value(json).unwrap();
    assert!(a.update_rotation);
    assert_eq!((a.angular_speed, a.angular_acceleration), (120.0, 720.0));
}
