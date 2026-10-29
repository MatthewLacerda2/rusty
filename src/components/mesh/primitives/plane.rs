//! src/components/mesh/primitives/plane.rs — the `Plane` primitive: one quad, or a
//! grid of them (#1524) for a surface whose vertices move (`vertex_waves`).

use super::{fill_tangents, Vertex};
use glam::Vec3;

/// The most quads per side a `Plane` takes: 257² vertices, a dense pool of water.
pub const MAX_PLANE_SUBDIVISIONS: u32 = 256;

/// Generates a flat quad aligned on the XZ plane
pub fn generate_plane(width: f32, depth: f32) -> (Vec<Vertex>, Vec<u32>) {
    let w = width / 2.0;
    let d = depth / 2.0;

    let norm = Vec3::new(0.0, 1.0, 0.0);

    let vertices = vec![
        Vertex::new(Vec3::new(-w, 0.0, -d), norm, [0.0, 0.0]),
        Vertex::new(Vec3::new(w, 0.0, -d), norm, [1.0, 0.0]),
        Vertex::new(Vec3::new(w, 0.0, d), norm, [1.0, 1.0]),
        Vertex::new(Vec3::new(-w, 0.0, d), norm, [0.0, 1.0]),
    ];
    // One quad over [0, 1]²: its texture UV is already a lightmap unwrap (#438).
    let vertices = vertices
        .into_iter()
        .map(|v| v.with_lightmap_uv(v.tex_coords))
        .collect();

    let indices = vec![0, 2, 1, 0, 3, 2];

    fill_tangents(vertices, indices)
}

/// [`generate_plane`] cut into `subdivisions` × `subdivisions` quads (clamped to
/// `1..=`[`MAX_PLANE_SUBDIVISIONS`]): `(n + 1)²` vertices over the same size and the
/// same UVs, each quad wound as the single one is. 1 is [`generate_plane`] itself,
/// vertex for vertex.
pub fn generate_plane_grid(width: f32, depth: f32, subdivisions: u32) -> (Vec<Vertex>, Vec<u32>) {
    let n = subdivisions.clamp(1, MAX_PLANE_SUBDIVISIONS);
    if n == 1 {
        return generate_plane(width, depth);
    }
    let norm = Vec3::new(0.0, 1.0, 0.0);
    let row = n + 1;
    let mut vertices = Vec::with_capacity((row * row) as usize);
    for j in 0..row {
        for i in 0..row {
            let uv = [i as f32 / n as f32, j as f32 / n as f32];
            let at = Vec3::new((uv[0] - 0.5) * width, 0.0, (uv[1] - 0.5) * depth);
            // The texture UV is already a lightmap unwrap, as the single quad's is (#438).
            vertices.push(Vertex::new(at, norm, uv).with_lightmap_uv(uv));
        }
    }
    let mut indices = Vec::with_capacity((n * n * 6) as usize);
    for j in 0..n {
        for i in 0..n {
            let a = j * row + i;
            let (b, c) = (a + 1, a + row);
            indices.extend_from_slice(&[a, c + 1, b, a, c, c + 1]);
        }
    }
    fill_tangents(vertices, indices)
}
