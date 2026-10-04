//! src/components/mesh/vertex.rs — the mesh vertex layout.
//!
//! Plain `#[repr(C)]` data shared by `MeshComponent`, the asset importers' rehydrate
//! path and the procedural primitives. The renderer uploads it as-is and owns the
//! matching GPU buffer layout (`render::gpu::mesh::vertex_layout`); this module
//! knows nothing about wgpu, so the sim can build and hold meshes headless (#494).

use glam::Vec3;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub joint_indices: [u32; 4],
    pub joint_weights: [f32; 4],
    /// Normal-map tangent basis: `xyz` unit tangent, `w` handedness (±1) (#207).
    pub tangent: [f32; 4],
    /// The lightmap UV (#438): glTF `TEXCOORD_1`, Blender's second UV map. All zeros
    /// on a mesh without one, which the lightmap bake reads as "no lightmap".
    pub lightmap_uv: [f32; 2],
}

impl Vertex {
    pub fn new(pos: Vec3, norm: Vec3, uv: [f32; 2]) -> Self {
        Self {
            position: pos.to_array(),
            normal: norm.to_array(),
            tex_coords: uv,
            joint_indices: [0, 0, 0, 0],
            joint_weights: [1.0, 0.0, 0.0, 0.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            lightmap_uv: [0.0, 0.0],
        }
    }

    /// The same vertex carrying a lightmap UV (#438).
    pub fn with_lightmap_uv(mut self, uv: [f32; 2]) -> Self {
        self.lightmap_uv = uv;
        self
    }
}
