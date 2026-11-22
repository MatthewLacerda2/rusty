use super::Vertex;
use glam::Vec3;
use rusty_asset::lightmap_uv::{unwrap_vertices, LightmapUvSettings};
use rusty_asset::tangents::generate_tangents;

mod capsule;
mod cylinder;
mod plane;
pub use capsule::generate_capsule;
pub use cylinder::generate_cylinder;
pub use plane::{generate_plane, generate_plane_grid, MAX_PLANE_SUBDIVISIONS};

pub fn generate_box(width: f32, height: f32, depth: f32) -> (Vec<Vertex>, Vec<u32>) {
    let w = width / 2.0;
    let h = height / 2.0;
    let d = depth / 2.0;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let uv_coords = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    for (i, &(p0, p1, p2, p3, norm)) in box_faces(w, h, d).iter().enumerate() {
        let base_idx = (i * 4) as u32;
        for (p, uv) in [p0, p1, p2, p3].into_iter().zip(uv_coords) {
            let lm = box_lightmap_uv(i, uv);
            vertices.push(Vertex::new(p, norm, uv).with_lightmap_uv(lm));
        }

        // Two triangles per quad face.
        let b = base_idx;
        indices.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    fill_tangents(vertices, indices)
}

/// Gutter round each face's lightmap cell, as a fraction of the cell: filtering and the
/// bake's dilation never bleed one face into the next.
const LIGHTMAP_CELL_PAD: f32 = 0.06;

fn box_lightmap_uv(face: usize, uv: [f32; 2]) -> [f32; 2] {
    let (col, row) = ((face % 3) as f32, (face / 3) as f32);
    let span = 1.0 - 2.0 * LIGHTMAP_CELL_PAD;
    [
        (col + LIGHTMAP_CELL_PAD + uv[0] * span) / 3.0,
        (row + LIGHTMAP_CELL_PAD + uv[1] * span) / 2.0,
    ]
}

fn box_faces(w: f32, h: f32, d: f32) -> [(Vec3, Vec3, Vec3, Vec3, Vec3); 6] {
    [
        (
            Vec3::new(-w, -h, d),
            Vec3::new(w, -h, d),
            Vec3::new(w, h, d),
            Vec3::new(-w, h, d),
            Vec3::new(0.0, 0.0, 1.0),
        ),
        (
            Vec3::new(w, -h, -d),
            Vec3::new(-w, -h, -d),
            Vec3::new(-w, h, -d),
            Vec3::new(w, h, -d),
            Vec3::new(0.0, 0.0, -1.0),
        ),
        (
            Vec3::new(w, -h, d),
            Vec3::new(w, -h, -d),
            Vec3::new(w, h, -d),
            Vec3::new(w, h, d),
            Vec3::new(1.0, 0.0, 0.0),
        ),
        (
            Vec3::new(-w, -h, -d),
            Vec3::new(-w, -h, d),
            Vec3::new(-w, h, d),
            Vec3::new(-w, h, -d),
            Vec3::new(-1.0, 0.0, 0.0),
        ),
        (
            Vec3::new(-w, h, d),
            Vec3::new(w, h, d),
            Vec3::new(w, h, -d),
            Vec3::new(-w, h, -d),
            Vec3::new(0.0, 1.0, 0.0),
        ),
        (
            Vec3::new(-w, -h, -d),
            Vec3::new(w, -h, -d),
            Vec3::new(w, -h, d),
            Vec3::new(-w, -h, d),
            Vec3::new(0.0, -1.0, 0.0),
        ),
    ]
}

pub fn generate_sphere(radius: f32, rings: u32, sectors: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let ring_step = 1.0 / (rings as f32);
    let sector_step = 1.0 / (sectors as f32);

    for r in 0..=rings {
        for s in 0..=sectors {
            let theta = (r as f32) * std::f32::consts::PI * ring_step;
            let phi = (s as f32) * 2.0 * std::f32::consts::PI * sector_step;

            let x = theta.sin() * phi.cos();
            let y = theta.cos();
            let z = theta.sin() * phi.sin();

            let norm = Vec3::new(x, y, z);
            let pos = norm * radius;
            let uv = [(s as f32) * sector_step, 1.0 - (r as f32) * ring_step];

            vertices.push(Vertex::new(pos, norm, uv));
        }
    }

    for r in 0..rings {
        for s in 0..sectors {
            let idx0 = r * (sectors + 1) + s;
            let idx1 = r * (sectors + 1) + (s + 1);
            let idx2 = (r + 1) * (sectors + 1) + s;
            let idx3 = (r + 1) * (sectors + 1) + (s + 1);

            indices.extend_from_slice(&[idx0, idx1, idx3, idx0, idx3, idx2]);
        }
    }

    fill_tangents_unwrapped(vertices, indices)
}

fn fill_tangents_unwrapped(vertices: Vec<Vertex>, indices: Vec<u32>) -> (Vec<Vertex>, Vec<u32>) {
    let with_uv = |v: &Vertex, uv| v.with_lightmap_uv(uv);
    let s = LightmapUvSettings::default();
    let unwrapped = unwrap_vertices(&vertices, &indices, |v| v.position, with_uv, &s);
    let (vertices, indices) = unwrapped.unwrap_or((vertices, indices));
    fill_tangents(vertices, indices)
}

fn fill_tangents(mut vertices: Vec<Vertex>, indices: Vec<u32>) -> (Vec<Vertex>, Vec<u32>) {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    let normals: Vec<[f32; 3]> = vertices.iter().map(|v| v.normal).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|v| v.tex_coords).collect();
    let tangents = generate_tangents(&positions, &normals, &uvs, &indices);
    for (v, t) in vertices.iter_mut().zip(tangents) {
        v.tangent = t;
    }
    (vertices, indices)
}
