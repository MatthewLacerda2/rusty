//! Primitive test levels for reflection-probe placement (#1584), built from static
//! box colliders: the issue's arena (a pool hall between a red and a blue room,
//! joined by doorways) and a Dust2-sized grid of rooms.

use glam::Vec3;

use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// Wall thickness and height, world units.
pub const WALL: f32 = 0.3;
pub const WALL_HEIGHT: f32 = 4.0;
/// A doorway's width.
pub const DOOR: f32 = 2.0;

/// A static box collider spanning `[min, max]`.
pub fn block(scene: &mut Scene, min: Vec3, max: Vec3) {
    let id = scene.add_entity("Block".to_string());
    scene.world.set_static(id, true);
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = (min + max) * 0.5;
    }
    scene.world.set_collider(
        id,
        Some(ColliderComponent {
            shape: ColliderShape::Box { size: max - min },
            ..crate::scene::authoring::default_collider()
        }),
    );
}

/// A wall along x at `z` from `x0` to `x1`, or along z at `x` from `z0` to `z1`,
/// with a doorway centred at `door` when given.
fn wall(scene: &mut Scene, along_x: bool, at: f32, from: f32, to: f32, door: Option<f32>) {
    let mut piece = |a: f32, b: f32| {
        if b - a <= 0.0 {
            return;
        }
        let (lo, hi) = match along_x {
            true => (
                Vec3::new(a, 0.0, at - WALL * 0.5),
                Vec3::new(b, WALL_HEIGHT, at + WALL * 0.5),
            ),
            false => (
                Vec3::new(at - WALL * 0.5, 0.0, a),
                Vec3::new(at + WALL * 0.5, WALL_HEIGHT, b),
            ),
        };
        block(scene, lo, hi);
    };
    match door {
        Some(d) => {
            piece(from, d - DOOR * 0.5);
            piece(d + DOOR * 0.5, to);
        }
        None => piece(from, to),
    }
}

/// A floor slab, top at y = 0.
fn slab(scene: &mut Scene, x0: f32, z0: f32, x1: f32, z1: f32) {
    block(scene, Vec3::new(x0, -0.2, z0), Vec3::new(x1, 0.0, z1));
}

/// The issue's arena, ≈ 44 × 52 m: an open pool hall over x ∈ [-10, 10],
/// z ∈ [-26, 26], with a roofed red room (x ∈ [-22, -10]) and blue room
/// (x ∈ [10, 22]), each z ∈ [-6, 6] and 4 m high, joined to the hall by a doorway
/// at z = 0.
pub fn arena() -> Scene {
    let mut scene = Scene::new();
    slab(&mut scene, -10.0, -26.0, 10.0, 26.0);
    for side in [-1.0f32, 1.0] {
        let (near, far) = (10.0 * side, 22.0 * side);
        let (x0, x1) = (near.min(far), near.max(far));
        slab(&mut scene, x0, -6.0, x1, 6.0);
        wall(&mut scene, false, far, -6.0, 6.0, None);
        wall(&mut scene, true, -6.0, x0, x1, None);
        wall(&mut scene, true, 6.0, x0, x1, None);
        let roof = (
            Vec3::new(x0, WALL_HEIGHT, -6.0),
            Vec3::new(x1, WALL_HEIGHT + 0.2, 6.0),
        );
        block(&mut scene, roof.0, roof.1);
        // The hall's side wall, with the doorway into this room.
        wall(&mut scene, false, near, -26.0, 26.0, Some(0.0));
    }
    wall(&mut scene, true, -26.0, -10.0, 10.0, None);
    wall(&mut scene, true, 26.0, -10.0, 10.0, None);
    scene
}

/// A Dust2-sized level: `n × n` open rooms of `size` metres on one slab, every
/// wall between neighbours cut by a doorway.
pub fn grid_of_rooms(n: usize, size: f32) -> Scene {
    let mut scene = Scene::new();
    let extent = n as f32 * size;
    slab(&mut scene, 0.0, 0.0, extent, extent);
    for k in 0..=n {
        let at = k as f32 * size;
        let inner = (k > 0 && k < n).then_some(());
        for r in 0..n {
            let (a, b) = (r as f32 * size, (r + 1) as f32 * size);
            let door = inner.map(|_| (a + b) * 0.5);
            wall(&mut scene, true, at, a, b, door);
            wall(&mut scene, false, at, a, b, door);
        }
    }
    scene
}
