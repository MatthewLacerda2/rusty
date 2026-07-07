use std::cell::RefCell;
use std::rc::Rc;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;
use crate::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;
use glam::Vec3;

/// #31: a script's `Physics.Raycast` and the engine's `cast_ray` resolve to
/// the *same* entity for the same ray, because both go through the one rapier
/// world. The binding casts against the live world shared via the handle.
#[test]
fn raycast_matches_engine_cast_through_rapier() {
    let mut raw = Scene::new();
    let target = raw.add_entity("Target".to_string());
    raw.world.transform_mut(target).unwrap().position = Vec3::new(0.0, 0.0, 5.0);
    raw.world.set_static(target, true);
    raw.world.set_collider(target, Some(ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::splat(2.0),
        },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    });
    let scene = Rc::new(RefCell::new(raw));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -10.0, 10.0, -10.0, 10.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    let mut m = ScriptManager::new(Rc::clone(&scene), input, nav, console, camera, time);
    let physics = Rc::new(RefCell::new(Some(PhysicsWorld::from_scene(
        &scene.borrow(),
    ))));
    m.init_runtime(&physics).expect("runtime inits");

    // Engine path: cast straight down +Z, expect the target.
    let engine = physics
        .borrow()
        .as_ref()
        .unwrap()
        .cast_ray(Vec3::ZERO, Vec3::Z, f32::MAX)
        .map(|(id, _)| id);
    assert_eq!(engine, Some(target), "engine cast should hit the target");

    // Script path: the binding returns (hit, id, dist); pull the id back out.
    let lua_id: u32 = m
        .eval("select(2, Physics.Raycast(0,0,0, 0,0,1))")
        .unwrap()
        .split(',')
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(Some(lua_id), engine, "script raycast must match the engine");
}

/// A `ScriptManager` with its runtime live over `scene` plus a dynamic rigidbody
/// on entity `id`, ready for the `Physics.*` rigidbody setters. Keeps the physics
/// handle alive for the manager's borrow.
fn manager_with_dynamic_body() -> (ScriptManager, Rc<RefCell<Option<PhysicsWorld>>>, u32) {
    let mut raw = Scene::new();
    let id = raw.add_entity("Body".to_string());
    raw.world.set_rigidbody(id, Some(RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: CollisionDetection::Discrete,
    });
    let scene = Rc::new(RefCell::new(raw));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -10.0, 10.0, -10.0, 10.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    let mut m = ScriptManager::new(Rc::clone(&scene), input, nav, console, camera, time);
    let physics = Rc::new(RefCell::new(Some(PhysicsWorld::from_scene(
        &scene.borrow(),
    ))));
    m.init_runtime(&physics).expect("runtime inits");
    (m, physics, id)
}

/// #319: `Physics.SetAngularVelocity` writes the component and
/// `Physics.GetAngularVelocity` reads it back — a plain binding round-trip.
#[test]
fn angular_velocity_round_trips_through_the_binding() {
    let (m, _physics, id) = manager_with_dynamic_body();
    m.eval(&format!("Physics.SetAngularVelocity({id}, 1, 4, -2)"))
        .unwrap();
    let y: f32 = m
        .eval(&format!("({{Physics.GetAngularVelocity({id})}})[2]"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(y, 4.0, "the y-axis spin round-trips");
}

/// #321: `Physics.SetCollisionDetection` accepts the two mode names and
/// `Physics.GetCollisionDetection` echoes the stored mode; an unknown name errors.
#[test]
fn collision_detection_mode_round_trips_and_rejects_garbage() {
    let (m, _physics, id) = manager_with_dynamic_body();
    assert_eq!(
        m.eval(&format!("Physics.GetCollisionDetection({id})"))
            .unwrap()
            .trim(),
        "Discrete",
        "defaults to Discrete"
    );
    m.eval(&format!(
        "Physics.SetCollisionDetection({id}, 'Continuous')"
    ))
    .unwrap();
    assert_eq!(
        m.eval(&format!("Physics.GetCollisionDetection({id})"))
            .unwrap()
            .trim(),
        "Continuous"
    );
    assert!(
        m.eval(&format!("Physics.SetCollisionDetection({id}, 'Nope')"))
            .is_err(),
        "an unknown mode is a script error"
    );
}
