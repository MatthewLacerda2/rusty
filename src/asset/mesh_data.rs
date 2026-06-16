//! src/asset/mesh_data.rs — the importer's plain-data mesh types.
//!
//! These are deliberately decoupled from the renderer: an `ImportedAsset` is pure
//! geometry + material values (glam only), never a GPU `Vertex` or a wgpu buffer.
//! The scene layer converts a `MeshVertex` into the render `Vertex` when it
//! rehydrates a mesh, so the importer never depends on wgpu/egui/mlua.

use glam::Vec3;

/// One vertex of imported geometry: position, normal and a single UV channel.
/// Skin weights (joints) are intentionally absent here — they land with the
/// skinning issue (#79); this foundation is static meshes + materials only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
}

impl MeshVertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], tex_coords: [f32; 2]) -> Self {
        Self {
            position,
            normal,
            tex_coords,
        }
    }
}

/// A material's scalar values, as read from the source file. Texture images are
/// not loaded here (a later issue); this carries only the values needed to tint a
/// mesh and to round-trip the material's identity/name.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData {
    pub name: String,
    /// Linear RGBA base color factor (glTF `pbrMetallicRoughness.baseColorFactor`
    /// or OBJ `Kd` with alpha 1.0). Defaults to opaque white.
    pub base_color: [f32; 4],
}

impl Default for MaterialData {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// One addressable sub-object of a source file, e.g. `crates.glb::Barrel`.
///
/// `id` is the stable, human-readable handle used in scene references; it is the
/// source mesh's name when present, else a deterministic fallback (`mesh_<n>`).
/// Geometry is plain `MeshVertex` + index data; `material` indexes into the parent
/// `ImportedAsset::materials`, or `None` when the sub-mesh declares no material.
#[derive(Clone, Debug, PartialEq)]
pub struct SubMesh {
    pub id: String,
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

impl SubMesh {
    /// Axis-aligned bounds of this sub-mesh, or `None` when it has no vertices.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut iter = self.vertices.iter();
        let first = iter.next()?;
        let mut min = Vec3::from_array(first.position);
        let mut max = min;
        for v in iter {
            let p = Vec3::from_array(v.position);
            min = min.min(p);
            max = max.max(p);
        }
        Some((min, max))
    }
}

/// The full result of importing one source file: every addressable sub-mesh plus
/// the shared material table. This is the pure-data product the scene layer and
/// the Content Browser both consume.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedAsset {
    pub sub_meshes: Vec<SubMesh>,
    pub materials: Vec<MaterialData>,
}

impl ImportedAsset {
    /// Look up a sub-mesh by its `id` (the part after `::` in a reference).
    pub fn sub_mesh(&self, id: &str) -> Option<&SubMesh> {
        self.sub_meshes.iter().find(|m| m.id == id)
    }

    /// The ids of every sub-object, in file order — the sub-object map surfaced in
    /// the sidecar and the Content Browser.
    pub fn sub_mesh_ids(&self) -> Vec<String> {
        self.sub_meshes.iter().map(|m| m.id.clone()).collect()
    }
}
