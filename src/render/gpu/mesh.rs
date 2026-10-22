//! GPU vertex-buffer layout for the sim-side [`Vertex`] (#494): the vertex data and
//! the primitive builders live in `components::mesh`; the renderer only owns how that
//! data is described to wgpu, and the selection outline's smoothed normals (#1175).

use std::collections::BTreeMap;

use glam::Vec3;

use crate::components::mesh::Vertex;

const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
    0 => Float32x3,  // position
    1 => Float32x3,  // normal
    2 => Float32x2,  // tex_coords
    3 => Uint32x4,   // joint_indices
    4 => Float32x4,  // joint_weights
    5 => Float32x4,  // tangent (xyz + handedness)
    6 => Float32x2,  // lightmap_uv (#438)
];

/// The vertex-buffer layout every mesh pipeline binds at slot 0.
pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}

const OUTLINE_ATTRIBS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![
    7 => Float32x3,  // outline normal (#1175)
];

/// The selection outline's second vertex buffer, slot 1 (#1175): one
/// [`outline_normals`] entry per vertex. Only the outline pipelines bind it.
pub fn outline_normal_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &OUTLINE_ATTRIBS,
    }
}

/// The normals the selection outline grows `vertices` along (#1175): each vertex's
/// unit normal summed over every vertex at its exact position, renormalized. A
/// hard-edged mesh's faces then push out together and its outline shell stays
/// closed (a box corner's three face normals become its diagonal); a smooth mesh
/// keeps its normals. Where the normals cancel, a vertex keeps its own.
pub fn outline_normals(vertices: &[Vertex]) -> Vec<[f32; 3]> {
    // `+ 0.0` folds -0.0 into 0.0, so the two zeros share a position.
    let key = |v: &Vertex| v.position.map(|c| (c + 0.0).to_bits());
    let mut sums: BTreeMap<[u32; 3], Vec3> = BTreeMap::new();
    for v in vertices {
        *sums.entry(key(v)).or_default() += Vec3::from(v.normal).normalize_or_zero();
    }
    vertices
        .iter()
        .map(|v| {
            let own = Vec3::from(v.normal);
            sums[&key(v)].try_normalize().unwrap_or(own).to_array()
        })
        .collect()
}
