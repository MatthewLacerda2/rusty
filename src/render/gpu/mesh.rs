//! GPU vertex-buffer layout for the sim-side [`Vertex`] (#494): the vertex data and
//! the primitive builders live in `components::mesh`; the renderer only owns how that
//! data is described to wgpu.

use crate::components::mesh::Vertex;

const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
    0 => Float32x3,  // position
    1 => Float32x3,  // normal
    2 => Float32x2,  // tex_coords
    3 => Uint32x4,   // joint_indices
    4 => Float32x4,  // joint_weights
    5 => Float32x4,  // tangent (xyz + handedness)
];

/// The vertex-buffer layout every mesh pipeline binds at slot 0.
pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &ATTRIBS,
    }
}
