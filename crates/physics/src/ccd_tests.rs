//! Since rapier 0.36 every fast dynamic body is swept against static geometry whatever its mode; `Continuous` adds the sweep against moving bodies (Unity's Continuous / ContinuousDynamic split).

use glam::Vec3;

use super::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;
const WALL_X: f32 = 4.0;

fn add_wall(scene: &mut Scene, moving: bool) {
    let wall = scene.add_entity("Wall".to_string());
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(WALL_X, 0.0, 0.0);
    if moving {
        scene.world.set_rigidbody(
            wall,
            Some(RigidBodyComponent {
                is_kinematic: true,
                use_gravity: false,
                ..rusty_scene::create::defaults::default_rigidbody()
            }),
        );
    } else {
        scene.world.set_static(wall, true);
    }
    scene.world.set_collider(
        wall,
        Some(ColliderComponent {
            shape: ColliderShape::Box {
                size: Vec3::new(0.2, 4.0, 4.0),
            },
            ..rusty_scene::create::default_collider()
        }),
    );
}

fn wall_and_projectile(mode: CollisionDetection, moving: bool) -> (Scene, PhysicsWorld, u32) {
    let mut scene = Scene::new();
    add_wall(&mut scene, moving);

    let sphere = scene.add_entity("Bullet".to_string());
    scene.world.transform_mut(sphere).unwrap().position = Vec3::ZERO;
    scene.world.set_collider(
        sphere,
        Some(ColliderComponent {
            shape: ColliderShape::Sphere { radius: 0.25 },
            ..rusty_scene::create::default_collider()
        }),
    );
    scene.world.set_rigidbody(
        sphere,
        Some(RigidBodyComponent {
            // 300 m/s along +x ⇒ 5 m per tick; gravity off so motion stays on the
            // x axis and the test reads a single coordinate.
            velocity: Vec3::new(300.0, 0.0, 0.0),
            use_gravity: false,
            collision_detection: mode,
            ..rusty_scene::create::default_rigidbody()
        }),
    );

    let world = PhysicsWorld::from_scene(&scene);
    (scene, world, sphere)
}

fn final_x(mode: CollisionDetection, moving: bool) -> f32 {
    let (mut scene, mut world, sphere) = wall_and_projectile(mode, moving);
    for _ in 0..3 {
        world.step(&mut scene, DT);
    }
    scene.world.transform(sphere).map(|t| t.position.x).unwrap()
}

#[test]
fn a_static_wall_stops_the_sphere_in_either_mode() {
    for mode in [CollisionDetection::Discrete, CollisionDetection::Continuous] {
        let x = final_x(mode, false);
        assert!(
            x < WALL_X,
            "{mode:?} body should stop at the wall, ended at x={x}"
        );
    }
}

#[test]
fn discrete_tunnels_through_a_moving_wall() {
    let x = final_x(CollisionDetection::Discrete, true);
    assert!(
        x > WALL_X + 1.0,
        "Discrete body should tunnel past the wall, ended at x={x}"
    );
}

#[test]
fn continuous_stops_at_a_moving_wall() {
    let x = final_x(CollisionDetection::Continuous, true);
    assert!(
        x < WALL_X,
        "Continuous body should be stopped before the wall, ended at x={x}"
    );
}
