//! The batch key of one material slot of a solid (#470, #1615): what two draws must
//! share to be one instanced call — its shader, cull state, mesh run, material group
//! and record, and palette.

use super::batch::BatchKey;
use super::materials::entity_params;
use crate::components::MaterialAsset;
use crate::render::gpu::pipelines::Facing;
use crate::render::{MeshId, Renderer};
use crate::scene::Scene;

impl Renderer {
    /// The batch key of entity `id`'s material `slot`, drawing `indices` of its mesh
    /// placed by `world`, with its palette at `bone_base`; and whether that material
    /// is transparent.
    pub(super) fn slot_key(
        &mut self,
        scene: &Scene,
        id: u32,
        (slot, indices): (usize, std::ops::Range<u32>),
        (world, mesh, bone_base): (glam::Mat4, &MeshId, u32),
    ) -> (BatchKey, bool) {
        let entry = scene.material_entry_at(id, slot);
        let material = entry.map(|(_, asset)| asset);
        let record = self.solid_material(scene, id, material);
        let shader = material.and_then(|m| m.shader.as_deref());
        let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
        let key = BatchKey {
            pipeline,
            facing: Facing::of_material(world, material),
            mesh: mesh.clone(),
            indices: indices.into(),
            material: self.material_index(entry, pipeline, entity_params(scene, id, slot)),
            record: record.words(),
            bone_base,
        };
        (key, material.is_some_and(MaterialAsset::is_transparent))
    }
}
