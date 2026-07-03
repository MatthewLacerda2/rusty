//! Kinematic-body gravity (#318): a `use_gravity` kinematic character falls,
//! lands on static ground, rests without jitter, and starts a *fresh* fall when
//! stepping off a ledge (grounded contact zeroes the accumulated fall speed).

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, CollisionDetection, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn box_collider(size: Vec3) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

fn kinematic_body(use_gravity: bool) -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: true,
        mass: 1.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity,
        collision_detection: CollisionDetection::Discrete,
    }
}

/// Static floor slab: top face at y = 0.5.
fn add_floor(scene: &mut Scene, size: Vec3) -> u32 {
    let id = scene.add_entity("Floor".to_string());
    let mut e = scene.get_entity_mut(id).unwrap();
    e.is_static = true;
    e.collider = Some(box_collider(size));
    id
}

/// Unit-box character at `pos`, kinematic, with the given gravity opt-in.
fn add_character(scene: &mut Scene, pos: Vec3, rb: Option<RigidBodyComponent>) -> u32 {
    let id = scene.add_entity("Character".to_string());
    let mut e = scene.get_entity_mut(id).unwrap();
    e.transform.position = pos;
    e.collider = Some(box_collider(Vec3::ONE));
    e.rigidbody = rb;
    id
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.get_entity(id).unwrap().transform.position
}

#[test]
fn kinematic_body_with_gravity_falls() {
    let mut scene = Scene::new();
    let id = add_character(
        &mut scene,
        Vec3::new(0.0, 10.0, 0.0),
        Some(kinematic_body(true)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    // Semi-implicit Euler over n ticks drops g·dt²·n(n+1)/2 ≈ 4.99 m after 1 s.
    // Pinning the magnitude (not just "went down") catches a broken integration.
    let drop = 10.0 - pos_of(&scene, id).y;
    assert!(
        (drop - 4.99).abs() < 0.1,
        "1 s free fall ≈ 4.99 m, got {drop}"
    );
}

#[test]
fn kinematic_body_lands_and_rests_on_ground() {
    let mut scene = Scene::new();
    add_floor(&mut scene, Vec3::new(20.0, 1.0, 20.0));
    let id = add_character(
        &mut scene,
        Vec3::new(0.0, 5.0, 0.0),
        Some(kinematic_body(true)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..300 {
        physics.step(&mut scene, DT);
    }
    // Landed: floor top 0.5 + character half-height 0.5 (+ controller offset).
    let rested = pos_of(&scene, id).y;
    assert!(
        (0.99..=1.10).contains(&rested),
        "should rest on the floor, got y={rested}"
    );
    // Grounded: it stays put — no sinking, no downward-speed jitter.
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    let still = pos_of(&scene, id).y;
    assert!(
        (still - rested).abs() < 1e-3,
        "resting body must not jitter/sink, got y={still}"
    );
}

#[test]
fn grounded_body_starts_a_fresh_fall_off_a_ledge() {
    let mut scene = Scene::new();
    // Narrow platform: x spans [-2, 2].
    add_floor(&mut scene, Vec3::new(4.0, 1.0, 4.0));
    let id = add_character(
        &mut scene,
        Vec3::new(0.0, 1.2, 0.0),
        Some(kinematic_body(true)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    // Rest on the platform long enough that un-zeroed gravity would have built
    // a large fall speed (~20 m/s after 2 s).
    for _ in 0..120 {
        physics.step(&mut scene, DT);
    }
    let rested = pos_of(&scene, id).y;
    // Step off the ledge (a script teleport) and tick once: the first tick of a
    // *fresh* fall drops ~g·dt² ≈ 0.003 m; a carried speed would plummet ~0.33 m.
    scene.get_entity_mut(id).unwrap().transform.position.x = 5.0;
    physics.step(&mut scene, DT);
    let after = pos_of(&scene, id).y;
    assert!(
        rested - after < 0.05,
        "grounded ticks must not bank fall speed, dropped {}",
        rested - after
    );
    // And off the ledge it does keep falling.
    for _ in 0..120 {
        physics.step(&mut scene, DT);
    }
    assert!(
        pos_of(&scene, id).y < -5.0,
        "off the ledge the body falls freely"
    );
}

#[test]
fn gravity_needs_a_rigidbody_opting_in() {
    let mut scene = Scene::new();
    // No rigidbody at all (collider-only scenery) and an explicit opt-out: neither falls.
    let bare = add_character(&mut scene, Vec3::new(0.0, 10.0, 0.0), None);
    let opted_out = add_character(
        &mut scene,
        Vec3::new(3.0, 10.0, 0.0),
        Some(kinematic_body(false)),
    );
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    assert!(
        (pos_of(&scene, bare).y - 10.0).abs() < 1e-3,
        "no rigidbody: never falls"
    );
    assert!(
        (pos_of(&scene, opted_out).y - 10.0).abs() < 1e-3,
        "use_gravity=false: holds altitude"
    );
    // Flipping the authored flag at runtime takes effect next tick.
    scene
        .get_entity_mut(opted_out)
        .unwrap()
        .rigidbody
        .as_mut()
        .unwrap()
        .use_gravity = true;
    for _ in 0..60 {
        physics.step(&mut scene, DT);
    }
    assert!(
        pos_of(&scene, opted_out).y < 9.0,
        "enabling use_gravity starts the fall"
    );
}
