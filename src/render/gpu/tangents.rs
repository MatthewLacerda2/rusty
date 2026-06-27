//! Fill render `Vertex` tangents for the procedural primitives.
//!
//! The primitive builders (`generate_box`, `generate_sphere`, …) construct vertices
//! with the `+X` tangent placeholder; this derives the real tangent basis from their
//! positions + UVs via the asset layer's pure [`generate_tangents`] so primitives
//! sample normal maps correctly, exactly like an imported mesh (#207).

use crate::render::gpu::mesh::Vertex;
use crate::asset::tangents::generate_tangents;

/// Overwrite each vertex's `tangent` with the basis derived from positions + UVs. The
/// `generate_*` builders apply this before returning their geometry.
pub(crate) fn fill_tangents(
    mut vertices: Vec<Vertex>,
    indices: Vec<u32>,
) -> (Vec<Vertex>, Vec<u32>) {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    let normals: Vec<[f32; 3]> = vertices.iter().map(|v| v.normal).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|v| v.tex_coords).collect();
    let tangents = generate_tangents(&positions, &normals, &uvs, &indices);
    for (v, t) in vertices.iter_mut().zip(tangents) {
        v.tangent = t;
    }
    (vertices, indices)
}
