//! Mesh particles draw as solid entities (forward shader, shadows received, fog), one DrawItem each,
//! so a mesh and material batch into one instanced draw. Emitter colour and gradient do not apply;
//! they cast no shadows and take no decal stamp. Placed unmirrored: the material's face picks the cull.

use glam::{Mat4, Quat, Vec3};

use crate::draw::batch::sort::{view_depth, TransparentOrder};
use crate::draw::batch::{BatchKey, DrawItem};
use crate::draw::map_slots::material_maps;
use crate::draw::uniforms::{probe_occlusion_at, probe_sh_at};
use crate::gpu::pipelines::Facing;
use crate::gpu::uniforms::ObjectData;
use crate::passes::shadows::{depth_only, slot_clip};
use crate::scene::Variant;
use crate::{InstanceData, Renderer};
use rusty_components::{MaterialAsset, ParticleEmitterComponent, ParticleRenderMode};
use rusty_scene::create::{primitive_mesh_component, Primitive};
use rusty_scene::serialize::asset_mesh_component;
use rusty_scene::{Camera, MeshComponent, Scene};

type SolidLists<'a> = (
    &'a mut Vec<DrawItem>,
    &'a mut Vec<(DrawItem, TransparentOrder)>,
);

pub(super) fn mesh_for_spec(spec: &str) -> MeshComponent {
    Primitive::parse(spec)
        .and_then(primitive_mesh_component)
        .unwrap_or_else(|| asset_mesh_component(spec))
}

pub(crate) fn mesh_particle_matrix(position: Vec3, size: f32, orientation: Quat) -> Mat4 {
    Mat4::from_scale_rotation_translation(Vec3::splat(size), orientation, position)
}

impl Renderer {
    pub(crate) fn push_mesh_particles(
        &mut self,
        scene: &Scene,
        cam: &Camera,
        lists: SolidLists,
    ) -> u32 {
        let (opaque, transparent) = lists;
        let before = opaque.len() + transparent.len();
        for id in scene.world.ids_with_particles() {
            let emitter = scene.world.particles(id).expect("id has particles");
            if emitter.render.mode != ParticleRenderMode::Mesh {
                continue;
            }
            self.note_emitter_mesh(scene, &emitter);
            if emitter.runtime.particles.is_empty()
                || !scene.world.is_active_in_hierarchy(id)
                || !rusty_scene::layer_in_mask(scene.world.layer(id), cam.culling_mask)
            {
                continue;
            }
            let Some((key, glass)) = self.mesh_emitter_key(scene, &emitter) else {
                continue;
            };
            for p in &emitter.runtime.particles {
                let model = mesh_particle_matrix(p.position, emitter.size_of(p), p.orientation());
                let (use_sh, sh) = probe_sh_at(scene, p.position);
                let object = self.retained.objects.push_transient(ObjectData {
                    model_matrix: model.to_cols_array(),
                    use_sh,
                    sh,
                    occlusion: probe_occlusion_at(scene, p.position),
                    ..ObjectData::IDENTITY
                });
                let item = DrawItem {
                    key,
                    instance: InstanceData {
                        reflection_probe: self.reflection_probes.probe_for(p.position),
                        ..InstanceData::of(object)
                    },
                    depth: view_depth(p.position, cam.position, cam.forward()),
                    outline: false,
                };
                match glass {
                    Some(priority) => {
                        let depth = item.depth;
                        transparent.push((item, TransparentOrder { priority, depth }));
                    }
                    None => opaque.push(item),
                }
            }
        }
        (opaque.len() + transparent.len() - before) as u32
    }

    fn note_emitter_mesh(&mut self, scene: &Scene, emitter: &ParticleEmitterComponent) {
        if let Some((mesh, _)) = self.particle_mesh(emitter) {
            let handle = self.retained.meshes.intern(&mesh);
            self.note_particle_mesh(scene.id(), handle);
        }
    }

    fn mesh_emitter_key(
        &mut self,
        scene: &Scene,
        emitter: &ParticleEmitterComponent,
    ) -> Option<(BatchKey, Option<i32>)> {
        let (mesh, num_indices) = self.particle_mesh(emitter)?;
        let entry = emitter
            .render
            .material
            .as_ref()
            .and_then(|m| scene.materials.get_key_value(m))
            .map(|(key, asset)| (key.as_str(), asset));
        let material = entry.map(|(_, asset)| asset);
        let due = self.notes_due();
        for (path, kind) in material_maps(material) {
            if due {
                self.note_map(scene.id(), path.as_str());
            }
            self.load_texture_as(&path, kind);
        }
        let shader = material.and_then(|m| m.shader.as_deref());
        let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
        let (record, traits) = self.material_record(scene.id(), entry, Variant::Debris);
        let mut key = BatchKey {
            pipeline,
            facing: Facing::of_material(Mat4::IDENTITY, material),
            mesh: self.retained.meshes.intern(&mesh),
            indices: (0..num_indices).into(),
            material: self.material_index(scene.id(), entry, pipeline, None),
            record,
            traits,
            bone_base: 0,
            depth_only: false,
        };
        let clip = material.and_then(|m| slot_clip(&self.surface_shaders, &key, m));
        key.depth_only = depth_only(key.pipeline, clip.as_ref());
        let glass = material.filter(|m| m.is_transparent());
        Some((key, glass.map(MaterialAsset::sort_priority)))
    }
}

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;
