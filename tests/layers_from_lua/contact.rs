//! A matrix change made from Lua mid-play reaches the live colliders at once
//! (Unity's behaviour): a box resting on the floor falls through it the moment its
//! layer stops colliding with the floor's.

use std::cell::RefCell;

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape, RigidBodyComponent};
use rusty::physics::PhysicsWorld;
use rusty::scene::Scene;

use super::with_api;

const DT: f32 = 1.0 / 60.0;

fn body(scene: &mut Scene, pos: Vec3, size: Vec3, dynamic: bool) -> u32 {
    let id = scene.add_entity("Body".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(id, Some(collider));
    if dynamic {
        let rb = RigidBodyComponent {
            is_kinematic: false,
            ..rusty::scene::authoring::defaults::default_rigidbody()
        };
        scene.world.set_rigidbody(id, Some(rb));
    } else {
        scene.world.set_static(id, true);
    }
    id
}

#[test]
fn ignoring_a_pair_mid_play_drops_the_body_through_the_floor() {
    let mut scene = Scene::new();
    body(
        &mut scene,
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(10.0, 1.0, 10.0),
        false,
    );
    let crate_id = body(&mut scene, Vec3::new(0.0, 1.0, 0.0), Vec3::ONE, true);
    scene.world.set_layer(crate_id, 5);
    let mut world = PhysicsWorld::from_scene(&scene);
    for _ in 0..60 {
        world.step(&mut scene, DT);
    }
    let y = |s: &Scene| s.world.transform(crate_id).unwrap().position.y;
    assert!(
        (y(&scene) - 0.5).abs() < 0.05,
        "it rests on the floor first"
    );

    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load("Physics.IgnoreLayerCollision(5, 0)")
            .exec()
            .unwrap();
    });
    let mut scene = scene.into_inner();
    for _ in 0..60 {
        world.step(&mut scene, DT);
    }
    assert!(
        y(&scene) < -1.0,
        "no contact with the floor: it falls through"
    );
}
