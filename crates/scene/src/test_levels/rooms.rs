use glam::Vec3;

use super::block;
use crate::Scene;
use rusty_asset::mesh_data::Shared;
use rusty_components::mesh::{primitives, MeshRevision, Vertex};

const ROOM: f32 = 10.0;
const STOREY: f32 = 4.0;
const WALL: f32 = 0.2;
const DOOR: f32 = 1.4;

pub fn rooms(n: usize) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let extent = n as f32 * ROOM;
    let (vertices, indices) = primitives::generate_sphere(0.5, 24, 24);
    let sphere = (vertices.into(), indices.into());
    for storey in 0..2 {
        let y = storey as f32 * STOREY;
        for (i, j) in (0..n).flat_map(|i| (0..n).map(move |j| (i, j))) {
            let lo = Vec3::new(i as f32 * ROOM, y - WALL, j as f32 * ROOM);
            if storey == 0 || (i * n + j) % 7 != 3 {
                block(&mut scene, "Floor", lo, lo + Vec3::new(ROOM, WALL, ROOM));
            }
            props(&mut scene, lo + Vec3::Y * WALL, i + j + storey, &sphere);
        }
        for k in 0..=n {
            let at = k as f32 * ROOM;
            let inner = k > 0 && k < n;
            for r in 0..n {
                let (a, b) = (r as f32 * ROOM, (r + 1) as f32 * ROOM);
                let door = inner.then_some((a + b) * 0.5 + (k % 3) as f32 - 1.0);
                wall(&mut scene, true, (at, y), (a, b), door);
                wall(&mut scene, false, (at, y), (a, b), door);
            }
        }
    }
    let top = Vec3::new(0.0, 2.0 * STOREY - WALL, 0.0);
    block(
        &mut scene,
        "Roof",
        top,
        top + Vec3::new(extent, WALL, extent),
    );
    scene
}

fn wall(
    scene: &mut Scene,
    along_z: bool,
    (at, y): (f32, f32),
    (a, b): (f32, f32),
    door: Option<f32>,
) {
    let mut piece = |a: f32, b: f32| {
        let (lo, hi) = match along_z {
            true => (
                Vec3::new(at - WALL / 2.0, y, a),
                Vec3::new(at + WALL / 2.0, y + STOREY - WALL, b),
            ),
            false => (
                Vec3::new(a, y, at - WALL / 2.0),
                Vec3::new(b, y + STOREY - WALL, at + WALL / 2.0),
            ),
        };
        block(scene, "Wall", lo, hi);
    };
    match door {
        Some(d) => {
            piece(a, d - DOOR / 2.0);
            piece(d + DOOR / 2.0, b);
        }
        None => piece(a, b),
    }
}

fn props(scene: &mut Scene, lo: Vec3, k: usize, sphere: &(Shared<Vertex>, Shared<u32>)) {
    let shift = (k % 4) as f32;
    for at in [
        Vec3::new(2.0 + shift, 0.5, 2.5),
        Vec3::new(7.5, 0.5, 6.0 + shift * 0.5),
    ] {
        let at = lo + at;
        block(scene, "Crate", at - Vec3::splat(0.5), at + Vec3::splat(0.5));
    }
    let at = lo + Vec3::new(5.0 - shift, 0.75, 8.0);
    let id = block(
        scene,
        "Statue",
        at - Vec3::splat(0.75),
        at + Vec3::splat(0.75),
    );
    let mut mesh = scene.world.mesh_mut(id).expect("a box");
    (mesh.vertices, mesh.indices) = sphere.clone();
    mesh.revision = MeshRevision::next();
    mesh.own_geometry = Some(mesh.revision);
}
