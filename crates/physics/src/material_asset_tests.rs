//! crates/physics/src/material_asset_tests.rs — a shared physics material asset on the
//! solver (#1878): every collider naming it takes its values, and editing it
//! mid-play re-applies them on the next step without rebuilding a collider.

use glam::Vec3;

use super::PhysicsWorld;
use rusty_components::{ColliderComponent, ColliderShape, PhysicsMaterial};
use rusty_scene::physics_material;
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// Three static boxes: two naming `asset`, one with inline values.
fn boxes(asset: &str) -> (Scene, [u32; 3]) {
    let mut scene = Scene::new();
    let mut ids = [0; 3];
    for (i, id) in ids.iter_mut().enumerate() {
        *id = scene.add_entity(format!("Box{i}"));
        scene.world.transform_mut(*id).unwrap().position = Vec3::new(3.0 * i as f32, 0.0, 0.0);
        scene.world.set_static(*id, true);
        let collider = ColliderComponent {
            shape: ColliderShape::Box { size: Vec3::ONE },
            material: PhysicsMaterial::new(0.25, 0.0),
            material_asset: (i < 2).then(|| asset.to_string()),
            ..rusty_scene::authoring::default_collider()
        };
        scene.world.set_collider(*id, Some(collider));
    }
    (scene, ids)
}

fn friction(world: &PhysicsWorld, id: u32) -> f32 {
    world.colliders[world.id_to_collider[&id]].friction()
}

#[test]
fn editing_an_asset_retunes_every_collider_using_it() {
    let asset = rusty_test_support::temp::dir().join("ice.physmat");
    let asset = asset.to_string_lossy().into_owned();
    physics_material::write(&asset, PhysicsMaterial::new(0.05, 0.0)).unwrap();
    let (mut scene, [a, b, inline]) = boxes(&asset);
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    assert_eq!((friction(&world, a), friction(&world, b)), (0.05, 0.05));
    assert_eq!(friction(&world, inline), 0.25, "inline values stay its own");

    let handles = [a, b, inline].map(|id| world.id_to_collider[&id]);
    physics_material::write(&asset, PhysicsMaterial::new(0.8, 0.0)).unwrap();
    world.step(&mut scene, DT);
    assert_eq!((friction(&world, a), friction(&world, b)), (0.8, 0.8));
    assert_eq!(friction(&world, inline), 0.25);
    let after = [a, b, inline].map(|id| world.id_to_collider[&id]);
    assert_eq!(handles, after, "a re-sync, not a rebuild");
}

#[test]
fn a_dangling_asset_builds_with_the_default_material() {
    let missing = rusty_test_support::temp::dir().join("missing.physmat");
    let (mut scene, [a, _, _]) = boxes(&missing.to_string_lossy());
    let mut world = PhysicsWorld::from_scene(&scene);
    world.step(&mut scene, DT);
    assert_eq!(friction(&world, a), PhysicsMaterial::default().friction);
}
