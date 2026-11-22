//! Test hooks: the levels other crates' occlusion bake tests build.

use glam::Vec3;

use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

pub const HALF: f32 = 1.0;
pub const DEPTH: f32 = 8.0;
pub const LENGTH: f32 = 20.0;
pub const HEIGHT: f32 = 3.0;
pub(super) const WALL: f32 = 0.2;
pub(super) const DOOR: f32 = 1.4;

pub use rusty_scene::test_levels::block;

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

fn wall_z(scene: &mut Scene, x: f32, (z0, z1): (f32, f32)) {
    let (lo, hi) = (
        Vec3::new(x - WALL / 2.0, 0.0, z0),
        Vec3::new(x + WALL / 2.0, HEIGHT, z1),
    );
    block(scene, "Wall", lo, hi);
}

pub fn corridors() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let mut t = scene.world.transform_mut(sun).expect("a new entity");
    t.rotation = glam::Quat::from_rotation_x(-1.0);
    drop(t);
    let lamp = create_entity(&mut scene, "Lamp", Some(Primitive::PointLight));
    scene
        .world
        .transform_mut(lamp)
        .expect("a new entity")
        .position = Vec3::new(2.0, 2.5, 0.0);
    let (z0, z1) = (-HALF - DEPTH, HALF + DEPTH);
    let (lo, hi) = (Vec3::new(0.0, -0.2, z0), Vec3::new(LENGTH, 0.0, z1));
    block(&mut scene, "Floor", lo, hi);
    let up = Vec3::Y * (HEIGHT + 0.2);
    block(&mut scene, "Roof", lo + up, hi + up);
    for side in [-1.0f32, 1.0] {
        rooms(&mut scene, side);
    }
    wall_z(&mut scene, 0.0, (-HALF, HALF));
    wall_z(&mut scene, LENGTH, (-HALF, HALF));
    for x in [6.0, 14.0] {
        cube(&mut scene, "Crate", Vec3::new(x, 0.25, 0.4), 0.25);
    }
    scene
}

fn rooms(scene: &mut Scene, side: f32) {
    let half = LENGTH / 2.0;
    wall_x(scene, side * HALF, (0.0, half), Some(half / 2.0));
    wall_x(scene, side * HALF, (half, LENGTH), Some(half * 1.5));
    wall_x(scene, side * (HALF + DEPTH), (0.0, LENGTH), None);
    let span = match side < 0.0 {
        true => (-HALF - DEPTH, -HALF),
        false => (HALF, HALF + DEPTH),
    };
    for x in [0.0, half, LENGTH] {
        wall_z(scene, x, span);
    }
    for room in [0.0, half] {
        for (k, (dx, dz)) in [(2.0, 3.0), (7.0, 6.5), (4.5, 8.0)].into_iter().enumerate() {
            let at = Vec3::new(room + dx, 0.5, side * (HALF + dz - 0.5));
            cube(scene, &format!("Prop{k}"), at, 0.5);
        }
    }
}

fn cube(scene: &mut Scene, name: &str, at: Vec3, half: f32) -> u32 {
    block(scene, name, at - Vec3::splat(half), at + Vec3::splat(half))
}
