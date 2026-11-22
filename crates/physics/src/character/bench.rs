//! A test hook: the level `cost_tests` counts a `Move`'s work on and `dev`'s timed
//! `bench::move_cost_tests` runs, since a sim crate may not read a clock.

use glam::Vec3;

use super::move_character;
use crate::world::PhysicsWorld;
use rusty_components::{CharacterControllerComponent, ColliderComponent, SharedMesh};
use rusty_scene::Scene;

/// The side of one floor quad, metres.
const CELL: f32 = 0.5;
/// A script's ground stick each tick (`walker.lua`'s fall when nothing moves it).
const STICK: f32 = -0.040;
/// A walk's stride each tick: 4 m/s at the fixed 1/60 s, on a diagonal so it
/// crosses both the quads' edges and their diagonals.
const STRIDE: Vec3 = Vec3::new(0.047_14, 0.0, 0.047_14);

/// The floor (`cells` × `cells` quads, its top at y = 0, centred on the origin)
/// and the character standing on it at the centre, settled; its physics world.
pub fn level(cells: u32) -> (Scene, PhysicsWorld, u32) {
    let mut scene = Scene::new();
    let floor = scene.add_entity("floor".to_string());
    let half = cells as f32 * CELL * 0.5;
    let mut col = ColliderComponent::from_mesh_bounds(
        false,
        Vec3::new(-half, 0.0, -half),
        Vec3::new(half, 0.0, half),
    );
    col.shared_mesh = Some(SharedMesh {
        positions: grid(cells),
        indices: indices(cells).into(),
        ..Default::default()
    });
    scene.world.set_collider(floor, Some(col));
    let id = scene.add_entity("player".to_string());
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = Vec3::new(0.1, 1.2, 0.1);
    }
    let cc = CharacterControllerComponent::default();
    scene.world.set_character_controller(id, Some(cc));
    let physics = PhysicsWorld::from_scene(&scene);
    stand(&physics, &mut scene, id, 30);
    (scene, physics, id)
}

pub fn stand(physics: &PhysicsWorld, scene: &mut Scene, id: u32, ticks: usize) {
    for _ in 0..ticks {
        move_character(Some(physics), scene, id, Vec3::Y * STICK);
    }
}

pub fn walk(physics: &PhysicsWorld, scene: &mut Scene, id: u32, ticks: usize) {
    for _ in 0..ticks {
        move_character(Some(physics), scene, id, STRIDE + Vec3::Y * STICK);
    }
}

fn grid(cells: u32) -> Vec<[f32; 3]> {
    let half = cells as f32 * CELL * 0.5;
    let n = cells + 1;
    let at = |i: u32| i as f32 * CELL - half;
    (0..n * n).map(|k| [at(k % n), 0.0, at(k / n)]).collect()
}

fn indices(cells: u32) -> Vec<u32> {
    let n = cells + 1;
    let mut out = Vec::new();
    for z in 0..cells {
        for x in 0..cells {
            let a = z * n + x;
            let (b, c, d) = (a + 1, a + n, a + n + 1);
            out.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
    out
}
