//! Rigidbody angular velocity (#319): rapier round-trip through `PhysicsWorld`,
//! the kinematic/static no-op, and scene save/load persistence.

use glam::Vec3;
use rusty::components::{CollisionDetection, ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::authoring::rigidbody as rb_ops;
use rusty::scene::Scene;

fn box_collider() -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size: Vec3::ONE },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

fn dynamic_body(angular_velocity: Vec3) -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity,
        use_gravity: false,
        collision_detection: CollisionDetection::Discrete,
    }
}

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn dynamic_body_angular_velocity_survives_a_rapier_step() {
    // A free-spinning body under no torque or gravity keeps spinning at (about)
    // the rate it was given — this is the sim read-site `sync_from_rapier`
    // (`src/physics/world.rs`) writes back onto the component each tick.
    let mut scene = Scene::new();
    let id = scene.add_entity("Spinner".to_string());
    let spin = Vec3::new(0.0, 2.0, 0.0);
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        e.collider = Some(box_collider());
        e.rigidbody = Some(dynamic_body(spin));
    }
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        physics.step(&mut scene, 1.0 / 60.0);
    }
    let rb = scene.get_entity(id).unwrap().rigidbody.clone().unwrap();
    assert!(rb.angular_velocity.is_finite(), "must stay a sane number");
    assert!(
        (rb.angular_velocity.y - spin.y).abs() < 0.1,
        "undisturbed spin should be conserved, got {:?}",
        rb.angular_velocity
    );

    // A script can inject a new angular velocity mid-flight (mirrors SetVelocity).
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        rb_ops::set_angular_velocity(&mut e, Vec3::new(0.0, -1.0, 0.0));
    }
    physics.step(&mut scene, 1.0 / 60.0);
    let rb = scene.get_entity(id).unwrap().rigidbody.clone().unwrap();
    assert!(rb.angular_velocity.y < 0.0, "the injected spin took effect");
}

#[test]
fn angular_velocity_is_a_no_op_on_kinematic_and_static_bodies() {
    let mut scene = Scene::new();

    let kinematic = scene.add_entity("Kinematic".to_string());
    {
        let mut e = scene.get_entity_mut(kinematic).unwrap();
        e.collider = Some(box_collider());
        e.rigidbody = Some(RigidBodyComponent {
            is_kinematic: true,
            ..dynamic_body(Vec3::ZERO)
        });
        rb_ops::set_angular_velocity(&mut e, Vec3::new(1.0, 1.0, 1.0));
    }
    assert_eq!(
        scene
            .get_entity(kinematic)
            .unwrap()
            .rigidbody
            .as_ref()
            .unwrap()
            .angular_velocity,
        Vec3::ZERO
    );

    let statik = scene.add_entity("Static".to_string());
    {
        let mut e = scene.get_entity_mut(statik).unwrap();
        e.is_static = true;
        e.collider = Some(box_collider());
        e.rigidbody = Some(dynamic_body(Vec3::ZERO));
        rb_ops::set_angular_velocity(&mut e, Vec3::new(1.0, 1.0, 1.0));
    }
    assert_eq!(
        scene
            .get_entity(statik)
            .unwrap()
            .rigidbody
            .as_ref()
            .unwrap()
            .angular_velocity,
        Vec3::ZERO
    );
}

#[test]
fn angular_velocity_round_trips_through_save_load() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Spinner".to_string());
    scene.get_entity_mut(id).unwrap().rigidbody = Some(dynamic_body(Vec3::new(0.5, 1.5, -2.5)));

    let path = tmp("rusty_angular_velocity_roundtrip.scene");
    scene.save_to_file(&path).unwrap();

    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    let rb = loaded.get_entity(id).unwrap().rigidbody.clone().unwrap();
    assert_eq!(rb.angular_velocity, Vec3::new(0.5, 1.5, -2.5));
}
