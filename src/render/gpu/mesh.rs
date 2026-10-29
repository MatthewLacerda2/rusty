//! GPU vertex-buffer layout for the sim-side [`Vertex`] (#494), and the [`MeshId`]
//! uploads are keyed by: the vertex data and the primitive builders live in
//! `components::mesh`; the renderer only owns how that data is described to wgpu.

use crate::components::mesh::Vertex;

/// Stable identity for a mesh's GPU geometry, derived from its *source*
/// (`primitive_type` + `asset_ref`) rather than the entity referencing it (#127).
/// Many entities sharing the same primitive or imported asset therefore map to a
/// single `MeshId` and share one vertex/index buffer pair instead of allocating
/// N copies. The string form keeps a future runtime-mutated mesh expressible with
/// its own unique key (e.g. by entity id) without colliding with the shared ones.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MeshId(pub String);

impl MeshId {
    /// Geometry key: `"<primitive_type>|<asset_ref>"`, plus `"|<subdivisions>"` for a
    /// Plane cut finer than one quad, so it never shares the 1-quad buffer (#1524).
    pub fn from_mesh(mesh: &crate::scene::MeshComponent) -> Self {
        let (kind, asset) = (&mesh.primitive_type, mesh.asset_ref.as_deref());
        let cuts = Some(mesh.subdivisions).filter(|&n| n != 1);
        let cuts = cuts.map(|n| format!("|{n}")).unwrap_or_default();
        MeshId(format!("{kind}|{}{cuts}", asset.unwrap_or("")))
    }
}

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
