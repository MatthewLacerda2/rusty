//! The batch key of one material slot of a solid (#470, #1615): what two draws must
//! share to be one instanced call — its shader, cull state, mesh run, material group
//! and record slot, and palette — and the record that slot holds.

use super::batch::{BatchKey, RecordTraits};
use super::materials::entity_params;
use crate::gpu::pipelines::Facing;
use crate::scene::{MeshHandle, Variant};
use crate::{MaterialData, Renderer};
use rusty_components::MaterialAsset;
use rusty_scene::{Scene, SceneId};

impl Renderer {
    /// The batch key of entity `id`'s material `slot`, drawing `indices` of its mesh
    /// `mesh` placed by `world`, with its palette at 0 (a camera places a skinned
    /// one's, `render::scene::draw`); and whether that material is transparent.
    pub(crate) fn slot_key(
        &mut self,
        scene: &Scene,
        id: u32,
        (slot, indices): (usize, std::ops::Range<u32>),
        (world, mesh): (glam::Mat4, MeshHandle),
    ) -> (BatchKey, bool) {
        let entry = scene.material_entry_at(id, slot);
        let material = entry.map(|(_, asset)| asset);
        // A light's own gizmo mesh draws its material unlit.
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
        };
        (key, material.is_some_and(MaterialAsset::is_transparent))
    }

    /// The slot of scene `scene`'s record of `material` — `(library key, asset)`,
    /// `None` for no material — under `variant`, kept in the material table
    /// (`render::scene::MaterialRecords`, #1706) and made to hold what it holds now
    /// ([`Self::variant_record`]); and what the CPU reads of it.
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

    /// `material`'s record under `variant`, its map channels set by what is
    /// resident ([`Self::with_map_channels`]).
    pub(crate) fn variant_record(
        &self,
        variant: Variant,
        material: Option<&MaterialAsset>,
    ) -> MaterialData {
        self.with_map_channels(variant.record(material), material)
    }
}
