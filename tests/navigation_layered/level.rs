//! A CS:GO-sized test level (#454): ~200 m square, three walkable levels, blocked out
//! from primitive boxes, slabs and tilted-box ramps only (no detailed geometry).
//!
//! A 5 × 5 grid of two-storey blocks, 24 m square each, 40 m apart: a first floor at
//! y = 4 over the whole footprint, a roof at y = 8 over its east half, four corner
//! pillars, a ramp from the street to the first floor and one from the first floor
//! to the roof. Long walls with door gaps split the streets, and a field of crates
//! sits between the blocks.

use glam::Vec3;
use rusty::navigation::NavBounds;
use rusty::scene::Scene;

use super::{add_box, add_ramp};

/// The level's XZ extent.
pub const BOUNDS: NavBounds = NavBounds::new(0.0, 200.0, 0.0, 200.0);

/// Build the level into a fresh scene.
pub fn build() -> Scene {
    let mut scene = Scene::new();
    add_box(
        &mut scene,
        Vec3::new(0.0, -0.5, 0.0),
        Vec3::new(200.0, 0.0, 200.0),
    );
    for bx in 0..5 {
        for bz in 0..5 {
            block(&mut scene, 14.0 + 40.0 * bx as f32, 10.0 + 40.0 * bz as f32);
        }
    }
    for row in 0..4 {
        let z = 37.0 + 40.0 * row as f32;
        for seg in 0..5 {
            let x = 40.0 * seg as f32;
            // A 34 m wall, then a 6 m doorway.
            add_box(
                &mut scene,
                Vec3::new(x, 0.0, z),
                Vec3::new(x + 34.0, 3.0, z + 1.0),
            );
        }
    }
    for i in 0..60 {
        // A fixed lattice of crates in the streets (no RNG: the layout is data).
        let (cx, cz) = (
            4.0 + 7.3 * (i % 25) as f32,
            4.5 + 40.0 * (i / 25) as f32 + 30.0,
        );
        add_box(
            &mut scene,
            Vec3::new(cx, 0.0, cz),
            Vec3::new(cx + 1.5, 1.2, cz + 1.5),
        );
    }
    scene
}

/// One two-storey block with its south-west corner at `(ox, oz)`.
fn block(scene: &mut Scene, ox: f32, oz: f32) {
    let floor1 = (Vec3::new(ox, 3.8, oz), Vec3::new(ox + 24.0, 4.0, oz + 24.0));
    add_box(scene, floor1.0, floor1.1);
    add_box(
        scene,
        Vec3::new(ox + 12.0, 7.8, oz),
        Vec3::new(ox + 24.0, 8.0, oz + 24.0),
    );
    for (px, pz) in [(0.0, 0.0), (23.0, 0.0), (0.0, 23.0), (23.0, 23.0)] {
        let p = Vec3::new(ox + px, 0.0, oz + pz);
        add_box(scene, p, p + Vec3::new(1.0, 7.8, 1.0));
    }
    add_ramp(scene, (ox - 10.0, 0.0), (ox, 4.0), (oz + 3.0, oz + 7.0));
    add_ramp(
        scene,
        (ox + 2.0, 4.0),
        (ox + 12.0, 8.0),
        (oz + 18.0, oz + 22.0),
    );
}
