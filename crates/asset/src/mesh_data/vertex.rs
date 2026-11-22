//! Plain `#[repr(C)]` data the renderer uploads as-is; no wgpu here, so the sim holds meshes
//! headless.

use glam::Vec3;

/// `joint_indices` index the sub-mesh's [`SkinData`](super::SkinData) joints; an unskinned vertex
/// is bone 0, full weight.
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

    pub fn with_lightmap_uv(mut self, uv: [f32; 2]) -> Self {
        self.lightmap_uv = uv;
        self
    }

    pub fn with_skin(mut self, joint_indices: [u32; 4], joint_weights: [f32; 4]) -> Self {
        self.joint_indices = joint_indices;
        self.joint_weights = joint_weights;
        self
    }
}
