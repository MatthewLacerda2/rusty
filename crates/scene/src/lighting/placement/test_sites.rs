//! Primitive sites for #1737's acceptance tests: a yard, a house on it whose roof
//! no one reaches (on the yard, or alone as an indoor level), and a pool. A test
//! hook: compiled in every build, since placement on a baked nav surface is
//! `navigation`'s tests' (`navigation::probe_site`).

use glam::Vec3;

use super::test_level::block;
use crate::authoring::{create_entity, Primitive};
use crate::Scene;
use rusty_components::{MaterialAsset, MaterialComponent, RenderMode};

/// A 30 × 30 m yard, top at y = 0.
pub fn yard() -> Scene {
    let mut scene = Scene::new();
    block(
        &mut scene,
        Vec3::new(-15.0, -0.2, -15.0),
        Vec3::new(15.0, 0.0, 15.0),
    );
    scene
}

/// A 10 × 10 m room at the yard's middle, 4 m high, 2 m thick walls, a 2 m door
/// on −z (under `RoomParams::doorway`, so the room is its own; a 3 m one read as a
/// doorway only on the old 1 m navmesh, #1887), under a flat roof as wide as its
/// walls: walkable, reached by nothing.
pub fn roofed_house(scene: &mut Scene) {
    let (o, i, h) = (7.0, 5.0, 4.0);
    block(scene, Vec3::new(-o, 0.0, i), Vec3::new(o, h, o));
    block(scene, Vec3::new(-o, 0.0, -i), Vec3::new(-i, h, i));
    block(scene, Vec3::new(i, 0.0, -i), Vec3::new(o, h, i));
    block(scene, Vec3::new(-o, 0.0, -o), Vec3::new(-1.0, h, -i));
    block(scene, Vec3::new(1.0, 0.0, -o), Vec3::new(o, h, -i));
    block(scene, Vec3::new(-o, h, -o), Vec3::new(o, h + 0.3, o));
}

/// The house alone, on a slab as wide as its walls: an indoor level whose roof
/// (14 × 14 m) is larger than its room (10 × 10 m).
pub fn indoor_house() -> Scene {
    let mut scene = Scene::new();
    block(
        &mut scene,
        Vec3::new(-7.0, -0.2, -7.0),
        Vec3::new(7.0, 0.0, 7.0),
    );
    roofed_house(&mut scene);
    scene
}

/// A 6 × 6 m pit 1.5 m deep in the yard's deck, water at deck level.
pub fn pool() -> Scene {
    let mut scene = Scene::new();
    let (d, r) = (-1.5, 15.0);
    block(&mut scene, Vec3::new(-r, -0.2, -r), Vec3::new(r, 0.0, -3.0));
    block(&mut scene, Vec3::new(-r, -0.2, 3.0), Vec3::new(r, 0.0, r));
    block(
        &mut scene,
        Vec3::new(-r, -0.2, -3.0),
        Vec3::new(-3.0, 0.0, 3.0),
    );
    block(
        &mut scene,
        Vec3::new(3.0, -0.2, -3.0),
        Vec3::new(r, 0.0, 3.0),
    );
    block(
        &mut scene,
        Vec3::new(-3.0, d - 0.2, -3.0),
        Vec3::new(3.0, d, 3.0),
    );
    let water = create_entity(&mut scene, "Water", Some(Primitive::Plane));
    scene.world.set_static(water, true);
    if let Some(mut t) = scene.world.transform_mut(water) {
        t.position = Vec3::new(0.0, -0.15, 0.0);
        t.scale = Vec3::new(6.0 / 15.0, 1.0, 6.0 / 15.0);
    }
    let look = MaterialAsset {
        render_mode: RenderMode::Transparent,
        alpha: 0.5,
        ..MaterialAsset::default()
    };
    scene.materials.insert("water".into(), look);
    scene
        .world
        .set_material(water, Some(MaterialComponent::new("water")));
    scene
}
