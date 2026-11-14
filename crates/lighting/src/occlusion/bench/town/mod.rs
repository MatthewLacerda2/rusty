//! A town the occlusion bake is measured on (#1981): `n × n` blocks of 10 m, each
//! a roofed building (walls with a doorway, `props` crates and a dense sphere
//! inside) or an open courtyard (a floor and the same props under the sky). A
//! courtyard's long sight lines over the roofs and down the streets beside
//! dense interiors is the shape a real level such as Kolvik has, and the
//! all-rooms `bench::level` has not.

use glam::Vec3;

use crate::occlusion::test_level::block;

pub mod authored;
use rusty_components::mesh::{primitives, MeshRevision};
use rusty_scene::Scene;

const BLOCK: f32 = 10.0;
const HEIGHT: f32 = 4.0;
const WALL: f32 = 0.2;
const DOOR: f32 = 1.4;

/// How a town's buildings and ground are made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Make {
    /// Primitive boxes, a wall piece or a floor each.
    Boxes,
    /// As an imported level is: each building one mesh, its walls and roof
    /// tessellated into triangles no two of which make a quad, on bumpy ground.
    Authored,
}

/// The town: `n × n` blocks, a block a courtyard when its hash falls under
/// `open` (0 none, 1 all), each with `props` crates and one sphere.
pub fn town(n: usize, open: f32, props: usize, make: Make) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    for (i, j) in (0..n).flat_map(|i| (0..n).map(move |j| (i, j))) {
        let lo = Vec3::new(i as f32 * BLOCK, -WALL, j as f32 * BLOCK);
        let floor = (lo, lo + Vec3::new(BLOCK, WALL, BLOCK));
        match make {
            Make::Boxes => _ = block(&mut scene, "Floor", floor.0, floor.1),
            Make::Authored => _ = authored::ground(&mut scene, floor.0, floor.1),
        }
        furnish(&mut scene, lo + Vec3::Y * WALL, props, i * 31 + j * 17);
        let hash = ((i * 7919 + j * 104_729) % 1000) as f32 / 1000.0;
        if hash >= open {
            let pieces = building(lo + Vec3::Y * WALL, i + j);
            match make {
                Make::Boxes => pieces
                    .iter()
                    .for_each(|b| _ = block(&mut scene, "Wall", b.0, b.1)),
                Make::Authored => _ = authored::mesh(&mut scene, "Building", &pieces),
            }
        }
    }
    scene
}

/// The boxes of a building on the block whose floor corner is `lo`: four walls
/// inset from the block's edge (a street round it), a doorway in one, and a roof.
fn building(lo: Vec3, k: usize) -> Vec<(Vec3, Vec3)> {
    let mut boxes = Vec::new();
    let (a, b) = (lo + Vec3::new(1.0, 0.0, 1.0), lo + Vec3::new(9.0, 0.0, 9.0));
    let top = HEIGHT - WALL;
    let door = k % 4;
    for side in 0..4 {
        let (from, to) = match side {
            0 => (a, Vec3::new(b.x, a.y, a.z)),
            1 => (Vec3::new(b.x, a.y, a.z), b),
            2 => (Vec3::new(a.x, a.y, b.z), b),
            _ => (a, Vec3::new(a.x, a.y, b.z)),
        };
        let mid = (from + to) * 0.5;
        let along = (to - from).normalize();
        let thick = Vec3::new(along.z.abs(), 0.0, along.x.abs()) * (WALL / 2.0);
        let mut piece = |p: Vec3, q: Vec3| {
            let (lo, hi) = (p.min(q) - thick, p.max(q) + thick + Vec3::Y * top);
            boxes.push((lo, hi));
        };
        match side == door {
            true => {
                piece(from, mid - along * (DOOR / 2.0));
                piece(mid + along * (DOOR / 2.0), to);
            }
            false => piece(from, to),
        }
    }
    let roof = Vec3::Y * top;
    let (rlo, rhi) = (a + roof - Vec3::new(0.1, 0.0, 0.1), b + roof);
    boxes.push((rlo, rhi + Vec3::new(0.1, WALL, 0.1)));
    boxes
}

/// `props` crates and a 1.5 m sphere of 1150 triangles on the block at `lo`.
fn furnish(scene: &mut Scene, lo: Vec3, props: usize, k: usize) {
    for p in 0..props {
        let h = (k * 13 + p * 7) % 49;
        let at = lo + Vec3::new(2.0 + (h % 7) as f32, 0.5, 2.0 + (h / 7) as f32);
        block(scene, "Crate", at - Vec3::splat(0.4), at + Vec3::splat(0.4));
    }
    let at = lo + Vec3::new(5.0, 0.75, 5.0);
    let id = block(
        scene,
        "Statue",
        at - Vec3::splat(0.75),
        at + Vec3::splat(0.75),
    );
    let (vertices, indices) = primitives::generate_sphere(0.5, 24, 24);
    let mut mesh = scene.world.mesh_mut(id).expect("a box");
    (mesh.vertices, mesh.indices) = (vertices, indices);
    mesh.revision = MeshRevision::next();
}
