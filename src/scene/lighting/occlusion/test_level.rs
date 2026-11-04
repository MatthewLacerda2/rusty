//! The corridors-and-rooms primitive level the occlusion tests bake and render
//! (#1714): a roofed corridor along +X, two rooms either side opening onto it by a
//! doorway each, and props (1 m boxes) in every room and down the corridor.

use glam::Vec3;

use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// The corridor's half width and the rooms' depth past its walls, metres.
pub const HALF: f32 = 1.0;
pub const DEPTH: f32 = 8.0;
/// The level's length along X and its height.
pub const LENGTH: f32 = 20.0;
pub const HEIGHT: f32 = 3.0;
const WALL: f32 = 0.2;
const DOOR: f32 = 1.4;

/// A static box over `[min, max]`.
pub fn block(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).expect("a new entity");
    (t.position, t.scale) = ((min + max) * 0.5, max - min);
    drop(t);
    scene.world.set_static(id, true);
    id
}

/// A wall along X at `z` over `[x0, x1]`, with a doorway centred at `door`.
fn wall_x(scene: &mut Scene, z: f32, (x0, x1): (f32, f32), door: Option<f32>) {
    let mut piece = |a: f32, b: f32| {
        if b > a {
            let (lo, hi) = (
                Vec3::new(a, 0.0, z - WALL / 2.0),
                Vec3::new(b, HEIGHT, z + WALL / 2.0),
            );
            block(scene, "Wall", lo, hi);
        }
    };
    match door {
        Some(d) => {
            piece(x0, d - DOOR / 2.0);
            piece(d + DOOR / 2.0, x1);
        }
        None => piece(x0, x1),
    }
}

/// A wall along Z at `x` over `[z0, z1]`.
fn wall_z(scene: &mut Scene, x: f32, (z0, z1): (f32, f32)) {
    let (lo, hi) = (
        Vec3::new(x - WALL / 2.0, 0.0, z0),
        Vec3::new(x + WALL / 2.0, HEIGHT, z1),
    );
    block(scene, "Wall", lo, hi);
}

/// The level, lit by a sun the roof shuts out: the props are where a test looks.
pub fn corridors() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene
        .world
        .transform_mut(sun)
        .expect("a new entity")
        .rotation = glam::Quat::from_rotation_x(-1.0);
    let lamp = create_entity(&mut scene, "Lamp", Some(Primitive::PointLight));
    scene
        .world
        .transform_mut(lamp)
        .expect("a new entity")
        .position = Vec3::new(2.0, 2.5, 0.0);
    let (z0, z1) = (-HALF - DEPTH, HALF + DEPTH);
    block(
        &mut scene,
        "Floor",
        Vec3::new(0.0, -0.2, z0),
        Vec3::new(LENGTH, 0.0, z1),
    );
    block(
        &mut scene,
        "Roof",
        Vec3::new(0.0, HEIGHT, z0),
        Vec3::new(LENGTH, HEIGHT + 0.2, z1),
    );
    let half = LENGTH / 2.0;
    for side in [-1.0f32, 1.0] {
        // The corridor wall, a door into each room, and the room's far wall.
        wall_x(&mut scene, side * HALF, (0.0, half), Some(half / 2.0));
        wall_x(&mut scene, side * HALF, (half, LENGTH), Some(half * 1.5));
        wall_x(&mut scene, side * (HALF + DEPTH), (0.0, LENGTH), None);
        let span = if side < 0.0 { (z0, -HALF) } else { (HALF, z1) };
        for x in [0.0, half, LENGTH] {
            wall_z(&mut scene, x, span);
        }
        for room in [0.0, half] {
            for (k, (dx, dz)) in [(2.0, 3.0), (7.0, 6.5), (4.5, 8.0)].into_iter().enumerate() {
                let at = Vec3::new(room + dx, 0.5, side * (HALF + dz - 0.5));
                block(
                    &mut scene,
                    &format!("Prop{k}"),
                    at - Vec3::splat(0.5),
                    at + Vec3::splat(0.5),
                );
            }
        }
    }
    wall_z(&mut scene, 0.0, (-HALF, HALF));
    wall_z(&mut scene, LENGTH, (-HALF, HALF));
    for x in [6.0, 14.0] {
        let at = Vec3::new(x, 0.25, 0.4);
        block(
            &mut scene,
            "Crate",
            at - Vec3::splat(0.25),
            at + Vec3::splat(0.25),
        );
    }
    scene
}
