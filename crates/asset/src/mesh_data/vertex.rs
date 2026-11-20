//! crates/asset/src/mesh_data/vertex.rs — the one mesh vertex layout.
//!
//! Plain `#[repr(C)]` data: what an import builds, what the import cache holds and
//! what every `MeshComponent` instancing it shares (`rusty_components::mesh::Vertex`
//! re-exports it). The renderer uploads it as-is and owns the matching GPU buffer
//! layout (`render::gpu::mesh::vertex_layout`); this module knows nothing about
//! wgpu, so the sim can build and hold meshes headless.

use glam::Vec3;

/// One vertex: position, normal, the texture and lightmap UVs, the tangent basis
/// and the skin binding. `joint_indices` index into the sub-mesh's
/// [`SkinData`](super::SkinData) joints (and thus the GPU bone palette);
/// `joint_weights` are the linear-blend weights. An unskinned vertex keeps the
/// identity binding (bone 0, full weight), which the shader leaves untouched.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub joint_indices: [u32; 4],
    pub joint_weights: [f32; 4],
    /// Normal-map tangent basis: `xyz` unit tangent, `w` handedness (±1). From the
    /// glTF `TANGENT` accessor when present, else generated at import.
    pub tangent: [f32; 4],
    /// The lightmap UV: glTF `TEXCOORD_1`, Blender's second UV map. All zeros on a
    /// mesh without one, which the lightmap bake reads as "no lightmap".
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

    /// The same vertex carrying a lightmap UV.
    pub fn with_lightmap_uv(mut self, uv: [f32; 2]) -> Self {
        self.lightmap_uv = uv;
        self
    }

    /// The same vertex bound to a skin (glTF `JOINTS_0`/`WEIGHTS_0`).
    pub fn with_skin(mut self, joint_indices: [u32; 4], joint_weights: [f32; 4]) -> Self {
        self.joint_indices = joint_indices;
        self.joint_weights = joint_weights;
        self
    }
}
