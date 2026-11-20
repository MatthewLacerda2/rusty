//! crates/asset/src/mesh_data/ — the importer's plain-data mesh types (a skin's in
//! `skin`).
//!
//! These are deliberately decoupled from the renderer: an `ImportedAsset` is pure
//! geometry + material values (glam only), never a wgpu buffer. Its [`Vertex`] is
//! the one every mesh draws, and a sub-mesh's vertex and index buffers are
//! [`Shared`]: a mesh instancing it holds the same allocation, copied only when one
//! instance is edited (`shared::edit`).

use super::anim_data::AnimationClip;
use super::convert::texture_ref::MapRef;
use super::material_range::MaterialRange;
use glam::Vec3;

pub mod shared;
mod skin;
mod vertex;

pub use shared::Shared;
pub use skin::{JointTransform, SkinData};
pub use vertex::Vertex;

/// A material's values, as read from the source file — the asset-layer DTO for the
/// full glTF 2.0 metallic-roughness model. Pure data (no GPU buffers): the scalar
/// factors plus the *resolved file paths* to the texture images. The scene layer
/// converts this into a renderable `MaterialAsset` (see `material_asset_from_import`).
///
/// glTF packs metallic + roughness into ONE `metallicRoughnessTexture` (metallic in
/// B, roughness in G), so [`MaterialData::metallic_roughness_map`] is a single path;
/// the conversion fans it out to the engine's separate metallic/roughness maps.
///
/// Only EXTERNAL texture URIs are resolved to paths here; embedded images
/// (`Source::View`, a data URI / buffer view) are left `None` (a documented
/// follow-up — extracting embedded pixels to disk is out of scope for #203).
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData {
    pub name: String,
    /// Linear RGBA base color factor (glTF `pbrMetallicRoughness.baseColorFactor`
    /// or OBJ `Kd` with alpha 1.0). Defaults to opaque white.
    pub base_color: [f32; 4],
    /// glTF `metallicFactor` (default 1.0 per the glTF spec).
    pub metallic: f32,
    /// glTF `roughnessFactor` (default 1.0 per the glTF spec).
    pub roughness: f32,
    /// Resolved path to the base-color (albedo) texture, or `None`.
    pub base_color_map: Option<MapRef>,
    /// Resolved path to the packed metallic-roughness texture (metallic=B,
    /// roughness=G), or `None`.
    pub metallic_roughness_map: Option<MapRef>,
    /// Resolved path to the normal map, or `None`.
    pub normal_map: Option<MapRef>,
    /// Resolved path to the emissive texture, or `None`.
    pub emissive_map: Option<MapRef>,
    /// glTF `emissiveFactor` (linear RGB, default `[0,0,0]`).
    pub emissive: [f32; 3],
    /// Resolved path to glTF's `occlusionTexture` (#2151, R), or `None`. An ORM
    /// file names the metallic-roughness image here too: one path, one upload.
    pub occlusion_map: Option<MapRef>,
    /// `occlusionTexture.strength`, 1 when absent, as glTF defaults it.
    pub occlusion_strength: f32,
    /// glTF `doubleSided` (#1615): both faces draw. `false` by default, as in glTF,
    /// and always for a `.blend`'s conversion (`gltf::material`, Unity's way).
    pub double_sided: bool,
    /// glTF `KHR_materials_ior`'s `ior` (#1998), 1.5 when absent, as the extension
    /// defaults it.
    pub ior: f32,
    /// `KHR_materials_specular`'s `specularFactor` (#1998), 1 when absent.
    pub specular: f32,
    /// Resolved path to `KHR_materials_specular`'s `specularTexture` (#2151, A), or
    /// `None`. Its `specularColorTexture` is not read: the material has no slot.
    pub specular_map: Option<MapRef>,
    /// `KHR_materials_specular`'s `specularColorFactor` (#1998), white when absent.
    pub specular_color: [f32; 3],
}

impl Default for MaterialData {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            // glTF spec defaults: a material with no factors is fully metallic/rough.
            metallic: 1.0,
            roughness: 1.0,
            base_color_map: None,
            metallic_roughness_map: None,
            normal_map: None,
            emissive_map: None,
            emissive: [0.0, 0.0, 0.0],
            occlusion_map: None,
            occlusion_strength: 1.0,
            double_sided: false,
            ior: 1.5,
            specular: 1.0,
            specular_map: None,
            specular_color: [1.0; 3],
        }
    }
}

/// One addressable sub-object of a source file, e.g. `crates.glb::Barrel`.
///
/// `id` is the stable, human-readable handle used in scene references; it is the
/// source mesh's name when present, else a deterministic fallback (`mesh_<n>`).
/// Geometry is [`Vertex`] + index data, each [`Shared`] with every mesh drawing it; `ranges` splits the index list into
/// runs, each drawn with its own material (#1285).
#[derive(Clone, Debug, PartialEq)]
pub struct SubMesh {
    pub id: String,
    pub vertices: Shared<Vertex>,
    pub indices: Shared<u32>,
    /// The runs of `indices` and the material each draws with, in index order and
    /// covering every index: Unity's sub-meshes. One run for a one-material mesh.
    pub ranges: Vec<MaterialRange>,
    /// The skeleton bound to this sub-mesh, when the source declared a glTF `skin`.
    /// `None` for static meshes (every `.obj`, and any glTF mesh without a skin);
    /// the per-vertex bindings then stay at their unskinned identity default.
    pub skin: Option<SkinData>,
    /// The animation clips that drive this sub-mesh's skeleton, in document order
    /// (#80). Empty for a static mesh or a skinned mesh the source never animates;
    /// each clip's tracks are keyed by the `skin`'s joint slots.
    pub clips: Vec<AnimationClip>,
    /// The name of the first glTF node that places this mesh, when it has one — the
    /// Blender *object* name, where `id` is the mesh-data name. The `_LOD<n>` import
    /// convention reads it (`asset::lod`, #472). `None` for `.obj`.
    pub node_name: Option<String>,
    /// Its clusters (#1968), built at import for a big static mesh, whose
    /// `indices` they reordered; `None` for the rest (`asset::clusters`).
    pub clusters: Option<std::sync::Arc<super::clusters::Clusters>>,
}

impl SubMesh {
    /// The material the first run draws with, which a one-material mesh is all of.
    #[cfg(test)]
    pub fn first_material(&self) -> Option<usize> {
        self.ranges.first().and_then(|r| r.material)
    }

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
/// the editor both consume.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedAsset {
    pub sub_meshes: Vec<SubMesh>,
    pub materials: Vec<MaterialData>,
    /// The file's node hierarchy (#1083), parents before children, skeleton
    /// joints left out (they spawn as the skinned mesh's bones, #453). Empty when
    /// the source has none (`.obj`); [`node_tree`](Self::node_tree) fills the gap.
    pub nodes: Vec<super::model_node::ModelNode>,
}

impl ImportedAsset {
    /// Look up a sub-mesh by its `id` (the part after `::` in a reference).
    pub fn sub_mesh(&self, id: &str) -> Option<&SubMesh> {
        self.sub_meshes.iter().find(|m| m.id == id)
    }

    /// The ids of every sub-object, in file order — the sub-object map surfaced in
    /// the sidecar and the model inspector.
    pub fn sub_mesh_ids(&self) -> Vec<String> {
        self.sub_meshes.iter().map(|m| m.id.clone()).collect()
    }
}
