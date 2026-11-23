use super::anim_data::AnimationClip;
use super::convert::texture_ref::MapRef;
use super::material_range::MaterialRange;
use glam::Vec3;

pub mod shared;
mod skin;
mod validate;
mod vertex;

pub use shared::Shared;
pub use skin::{JointTransform, SkinData};
pub use vertex::Vertex;

/// glTF packs metallic (B) and roughness (G) into one `metallicRoughnessTexture`,
/// so [`MaterialData::metallic_roughness_map`] is one path the scene layer fans
/// out. Embedded images (`Source::View`) are left `None`.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialData {
    pub name: String,
    /// Linear RGBA.
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub base_color_map: Option<MapRef>,
    pub metallic_roughness_map: Option<MapRef>,
    pub normal_map: Option<MapRef>,
    pub emissive_map: Option<MapRef>,
    pub emissive: [f32; 3],
    /// Channel R. An ORM file names the metallic-roughness image here too.
    pub occlusion_map: Option<MapRef>,
    pub occlusion_strength: f32,
    pub double_sided: bool,
    /// `KHR_materials_ior`; 1.5 when absent.
    pub ior: f32,
    /// `KHR_materials_specular`'s `specularFactor`; 1 when absent.
    pub specular: f32,
    /// Channel A.
    pub specular_map: Option<MapRef>,
    pub specular_color: [f32; 3],
}

impl Default for MaterialData {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            // glTF: no factors means fully metallic and rough.
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

/// `id` is the source mesh's name, else `mesh_<n>`.
#[derive(Clone, Debug, PartialEq)]
pub struct SubMesh {
    pub id: String,
    pub vertices: Shared<Vertex>,
    pub indices: Shared<u32>,
    /// Index order, covering every index: Unity's sub-meshes.
    pub ranges: Vec<MaterialRange>,
    pub skin: Option<SkinData>,
    pub clips: Vec<AnimationClip>,
    /// The Blender *object* name (`id` is the mesh-data name).
    pub node_name: Option<String>,
    /// Building them reorders `indices` (`clusters::apply`): the two go together.
    pub clusters: Option<std::sync::Arc<super::clusters::Clusters>>,
}

impl SubMesh {
    #[cfg(test)]
    pub fn first_material(&self) -> Option<usize> {
        self.ranges.first().and_then(|r| r.material)
    }

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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportedAsset {
    pub sub_meshes: Vec<SubMesh>,
    pub materials: Vec<MaterialData>,
    /// Parents before children, joints left out. Empty for `.obj`.
    pub nodes: Vec<super::model_node::ModelNode>,
}

impl ImportedAsset {
    pub fn sub_mesh(&self, id: &str) -> Option<&SubMesh> {
        self.sub_meshes.iter().find(|m| m.id == id)
    }

    pub fn sub_mesh_ids(&self) -> Vec<String> {
        self.sub_meshes.iter().map(|m| m.id.clone()).collect()
    }
}
