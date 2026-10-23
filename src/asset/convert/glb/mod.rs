//! src/asset/convert/glb/ — the glTF 2.0 a converter writes (#1024).
//!
//! A converter that reads its source itself (FBX through `ufbx`) fills a
//! [`Document`] and [`write()`] turns it into one binary glTF: geometry, base-colour
//! materials, the node hierarchy, skins and animations. Only what the engine's glTF importer
//! reads back is here, so the round trip `source → Document → .glb → ImportedAsset`
//! loses nothing the engine uses. Output is byte-for-byte deterministic: the same
//! document always writes the same file, so a versioned cache does not churn.
//!
//! A converter backed by another tool (Blender, #1072) writes its own glTF and
//! never touches this module.
// A converter's (#1172): a `let _ =` or a discarded `.ok()` here would drop a
// failure the cache must not be stamped over, and a print would bypass the one
// warning path; the rest of the crate keeps `let _ =` for best-effort cleanup.
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod anim;
mod bin;
mod read;
mod write;

pub use read::image_uris;
pub use write::write;

/// One glTF scene: a node forest plus the meshes, materials, skins and animations
/// it uses.
/// Indices are positions in these vectors, exactly as glTF stores them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub nodes: Vec<Node>,
    /// The scene's top-level nodes.
    pub roots: Vec<usize>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub skins: Vec<Skin>,
    pub animations: Vec<Animation>,
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

/// A metallic-roughness material.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub base_color_texture: Option<Texture>,
    pub normal_texture: Option<Texture>,
}

/// One texture a material samples: its image's URI, relative to the written
/// `.glb`'s directory and `/`-separated, and the image's name, when the
/// conversion gave it a stable identity (an extracted texture, #1130). One image
/// is written per distinct URI.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub uri: String,
    pub name: Option<String>,
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

/// One animation: named channels, each keying one path of one node, all sampled
/// linearly (the engine's importer reads them back as one clip).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Animation {
    pub name: String,
    pub channels: Vec<Channel>,
}

/// The keys of one node's translation, rotation or scale. `times` are seconds,
/// ascending, as many as there are values.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub node: usize,
    pub times: Vec<f32>,
    pub keys: Keys,
}

/// A channel's values, by the path they animate.
#[derive(Clone, Debug, PartialEq)]
pub enum Keys {
    Translation(Vec<[f32; 3]>),
    /// Unit quaternions, `[x, y, z, w]`.
    Rotation(Vec<[f32; 4]>),
    Scale(Vec<[f32; 3]>),
}

#[cfg(test)]
mod tests;
