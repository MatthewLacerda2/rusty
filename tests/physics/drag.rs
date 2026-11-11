//! Rigidbody drag (#2062): Unity's `Rigidbody.drag` / `angularDrag` reach
//! rapier's linear and angular damping, at body build and on a change in Play.

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::authoring::{default_collider, default_rigidbody};
use rusty::scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn collider(shape: ColliderShape) -> ColliderComponent {
    ColliderComponent {
        shape,
        ..default_collider()
    }
}

/// A ball (radius 0.4, a grenade or a can) on a wide static floor, rolling along
/// +X at 0.3 m/s. A ball and not a cylinder: rapier 0.36 feeds energy into a
/// cylinder rolling on a box, faster than Unity's default angular drag takes it
/// out, so a cylinder test would measure that, not drag (#2071).
fn rolling_ball(angular_drag: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".to_string());
    scene.world.transform_mut(floor).unwrap().position = Vec3::new(0.0, -0.5, 0.0);
    let size = Vec3::new(400.0, 1.0, 20.0);
    let shape = ColliderShape::Box { size };
    scene.world.set_collider(floor, Some(collider(shape)));
    scene.world.set_static(floor, true);
    let ball = scene.add_entity("Ball".to_string());
    scene.world.transform_mut(ball).unwrap().position = Vec3::new(0.0, 0.4, 0.0);
    let shape = ColliderShape::Sphere { radius: 0.4 };
    scene.world.set_collider(ball, Some(collider(shape)));
    let rb = RigidBodyComponent {
        velocity: Vec3::new(0.3, 0.0, 0.0),
        angular_velocity: Vec3::new(0.0, 0.0, -0.3 / 0.4), // rolling, not sliding
        angular_drag,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(ball, Some(rb));
    (scene, ball)
}

fn speed_after(scene: &mut Scene, id: u32, seconds: f32) -> f32 {
    let mut physics = PhysicsWorld::from_scene(scene);
    for _ in 0..(seconds / DT) as usize {
        physics.step(scene, DT);
    }
    scene.world.rigidbody(id).unwrap().velocity.length()
}

#[test]
fn a_rolling_ball_comes_to_rest_with_the_default_angular_drag() {
    assert_eq!(default_rigidbody().angular_drag, 0.05, "Unity's default");
    let (mut scene, ball) = rolling_ball(default_rigidbody().angular_drag);
    let speed = speed_after(&mut scene, ball, 180.0);
    assert!(speed < 0.01, "the ball still rolls at {speed} m/s");
}

#[test]
fn a_rolling_ball_without_angular_drag_rolls_on() {
    let (mut scene, ball) = rolling_ball(0.0);
    let speed = speed_after(&mut scene, ball, 180.0);
    assert!(speed > 0.29, "the ball slowed to {speed} m/s with no drag");
}

/// A body falling with `drag` d settles where damping cancels gravity's gain:
/// v = (v + g·dt) / (1 + dt·d) holds at v = g / d. The drag is set mid-fall, so
/// the change-in-Play path is what applies it.
#[test]
fn drag_set_in_play_gives_a_falling_body_a_terminal_speed() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Crate".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 1000.0, 0.0);
    let shape = ColliderShape::Box { size: Vec3::ONE };
    scene.world.set_collider(id, Some(collider(shape)));
    scene.world.set_rigidbody(id, Some(default_rigidbody()));
    let mut physics = PhysicsWorld::from_scene(&scene);
    let fall = |physics: &mut PhysicsWorld, scene: &mut Scene, seconds: f32| {
        for _ in 0..(seconds / DT) as usize {
            physics.step(scene, DT);
        }
        -scene.world.rigidbody(id).unwrap().velocity.y
    };
    let free = fall(&mut physics, &mut scene, 2.0);
    assert!(
        (free - 9.81 * 2.0).abs() < 0.5,
        "no drag: free fall, got {free}"
    );
    scene.world.rigidbody_mut(id).unwrap().drag = 2.0;
    let terminal = fall(&mut physics, &mut scene, 6.0);
    assert!(
        (terminal - 9.81 / 2.0).abs() < 0.05,
        "terminal speed {terminal}"
    );
}

/// A scene saved before drag existed loads with Unity's defaults.
#[test]
fn a_rigidbody_saved_without_drag_loads_unity_defaults() {
    let mut saved = serde_json::to_value(default_rigidbody()).unwrap();
    let fields = saved.as_object_mut().unwrap();
    fields.remove("drag");
    fields.remove("angular_drag");
    let rb: RigidBodyComponent = serde_json::from_value(saved).unwrap();
    assert_eq!((rb.drag, rb.angular_drag), (0.0, 0.05));
}
