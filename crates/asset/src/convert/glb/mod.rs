//! Output is byte-for-byte deterministic.
// A converter's: a `let _ =` or `.ok()` here would drop a failure the cache must not
// be stamped over; a print would bypass the one warning path.
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod anim;
mod bin;
mod read;
mod slots;
mod write;

pub(crate) use bin::Bin;
pub use read::{image_files, image_uris};
pub use slots::{image_slots, Slot};
pub use write::{container, write};

/// Indices are positions in these vectors, exactly as glTF stores them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub roots: Vec<usize>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub skins: Vec<Skin>,
    pub animations: Vec<Animation>,
}

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

/// Per-vertex streams are empty or as long as `positions`; `joints` and `weights`
/// come together or not at all.
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

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub base_color_texture: Option<Texture>,
    pub normal_texture: Option<Texture>,
    pub emissive_texture: Option<Texture>,
}

/// The URI is relative to the written `.glb`'s directory and `/`-separated; one image
/// is written per distinct URI.
#[derive(Clone, Debug, PartialEq)]
pub struct Texture {
    pub uri: String,
    pub name: Option<String>,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            name: String::new(),
            base_color: [1.0; 4],
            metallic: 1.0,
            roughness: 1.0,
            emissive: [0.0; 3],
            base_color_texture: None,
            normal_texture: None,
            emissive_texture: None,
        }
    }
}

/// Inverse bind matrices are column-major.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skin {
    pub joints: Vec<usize>,
    pub inverse_bind: Vec<[f32; 16]>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Animation {
    pub name: String,
    pub channels: Vec<Channel>,
}

/// `times` are seconds, ascending.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub node: usize,
    pub times: Vec<f32>,
    pub keys: Keys,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Keys {
    Translation(Vec<[f32; 3]>),
    /// Unit quaternions, `[x, y, z, w]`.
    Rotation(Vec<[f32; 4]>),
    Scale(Vec<[f32; 3]>),
}

#[cfg(test)]
mod tests;
