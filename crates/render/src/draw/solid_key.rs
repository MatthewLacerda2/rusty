use super::batch::{BatchKey, RecordTraits};
use super::materials::entity_params;
use crate::gpu::pipelines::Facing;
use crate::scene::{MeshHandle, Variant};
use crate::{MaterialData, Renderer};
use rusty_components::MaterialAsset;
use rusty_scene::{Scene, SceneId};

impl Renderer {
    pub(crate) fn slot_key(
        &mut self,
        scene: &Scene,
        id: u32,
        (slot, indices): (usize, std::ops::Range<u32>),
        (world, mesh): (glam::Mat4, MeshHandle),
    ) -> (BatchKey, bool) {
        let entry = scene.material_entry_at(id, slot);
        let material = entry.map(|(_, asset)| asset);
        let variant = match scene.world.has_light(id) {
            true => Variant::Unlit,
            false => Variant::Lit,
        };
        let (record, traits) = self.material_record(scene.id(), entry, variant);
        let shader = material.and_then(|m| m.shader.as_deref());
        let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
        let key = BatchKey {
            pipeline,
            facing: Facing::of_material(world, material),
            mesh,
            indices: indices.into(),
            material: self.material_index(
                scene.id(),
                entry,
                pipeline,
                entity_params(scene, id, slot),
            ),
            record,
            traits,
            bone_base: 0,
            depth_only: false,
        };
        (key, material.is_some_and(MaterialAsset::is_transparent))
    }

    pub(crate) fn material_record(
        &mut self,
        scene: SceneId,
        material: Option<(&str, &MaterialAsset)>,
        variant: Variant,
    ) -> (u32, RecordTraits) {
        let asset = material.map(|(_, asset)| asset);
        let record = self.variant_record(variant, asset);
        let name = material.map(|(key, _)| key);
        let slot = self.retained.records.slot((scene, name, variant), record);
        (slot, RecordTraits::of(&record))
    }

    pub(crate) fn variant_record(
        &self,
        variant: Variant,
        material: Option<&MaterialAsset>,
    ) -> MaterialData {
        self.with_map_channels(variant.record(material), material)
    }
}
