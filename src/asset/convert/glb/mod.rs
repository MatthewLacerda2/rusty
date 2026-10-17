//! src/asset/convert/glb/ — the glTF 2.0 a converter writes (#1024).
//!
//! A converter that reads its source itself (FBX through `ufbx`) fills a
//! [`Document`] and [`write`] turns it into one binary glTF: geometry, base-colour
//! materials, the node hierarchy and skins. Only what the engine's glTF importer
//! reads back is here, so the round trip `source → Document → .glb → ImportedAsset`
//! loses nothing the engine uses. Output is byte-for-byte deterministic: the same
//! document always writes the same file, so a versioned cache does not churn.
//!
//! A converter backed by another tool (Blender, #1072) writes its own glTF and
//! never touches this module.

mod write;

pub use write::write;

/// One glTF scene: a node forest plus the meshes, materials and skins it uses.
/// Indices are positions in these vectors, exactly as glTF stores them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub nodes: Vec<Node>,
    /// The scene's top-level nodes.
    pub roots: Vec<usize>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub skins: Vec<Skin>,
}

/// A node: a local transform, its children and what it places.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub translation: [f32; 3],
    /// A unit quaternion, `[x, y, z, w]`.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    pub children: Vec<usize>,
    pub mesh: Option<usize>,
    pub skin: Option<usize>,
}

impl Node {
    /// A node named `name` at the identity transform, placing nothing.
    pub fn new(name: impl Into<String>) -> Self {
        Node {
            name: name.into(),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
            children: Vec::new(),
            mesh: None,
            skin: None,
        }
    }
}

/// A mesh: one addressable sub-object once imported, its primitives split by
/// material.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

/// One indexed triangle list. Every per-vertex stream is either empty (absent) or
/// as long as `positions`; `joints` and `weights` are present together or not at
/// all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Primitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

/// A metallic-roughness material. Texture paths are URIs relative to the written
/// `.glb`'s directory, `/`-separated.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
}

impl Default for Material {
    /// glTF's own defaults: white, fully metallic, fully rough.
    fn default() -> Self {
        Material {
            name: String::new(),
            base_color: [1.0; 4],
            metallic: 1.0,
            roughness: 1.0,
            emissive: [0.0; 3],
            base_color_texture: None,
            normal_texture: None,
        }
    }
}

/// A skin: its joint nodes and, per joint, the inverse bind matrix (column-major)
/// that takes the skinned mesh's space into that joint's at bind time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skin {
    pub joints: Vec<usize>,
    pub inverse_bind: Vec<[f32; 16]>,
}

#[cfg(test)]
mod tests;
