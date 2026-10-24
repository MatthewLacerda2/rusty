//! src/render/passes/particles/mesh.rs — mesh particles (#440): shell casings,
//! debris, glass shards.
//!
//! A mesh particle is drawn exactly like a solid entity carrying that mesh and
//! material — same forward shader, PBR, probe ambient, shadows received, fog — by
//! handing the camera's solid list one [`DrawItem`] per particle. Particles of one
//! emitter share a mesh + material, so the batcher folds them into one instanced
//! draw. The particle's size scales the mesh uniformly and its `rotation` tumbles
//! it about its own random axis. The emitter's colour and gradient do not apply:
//! the look is the material's (a Transparent material sorts with the glass).
//! Mesh particles cast no shadows.

use glam::{Mat4, Quat, Vec3};

use crate::components::{MaterialAsset, ParticleEmitterComponent, ParticleRenderMode};
use crate::render::draw::batch::sort::view_depth;
use crate::render::draw::batch::{BatchKey, DrawItem};
use crate::render::draw::materials::material_maps;
use crate::render::draw::uniforms::{material_uniform, probe_sh_at};
use crate::render::{EntityUniform, InstanceData, Renderer};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::serialize::asset_mesh_component;
use crate::scene::{Camera, MeshComponent, Scene};

/// The camera's opaque items and its (unsorted) translucent items with their depth.
type SolidLists<'a> = (&'a mut Vec<DrawItem>, &'a mut Vec<(DrawItem, f32)>);

/// A mesh named by one string: a primitive (`"Box"`) or a model reference.
pub(super) fn mesh_for_spec(spec: &str) -> MeshComponent {
    Primitive::parse(spec)
        .and_then(primitive_mesh_component)
        .unwrap_or_else(|| asset_mesh_component(spec))
}

/// A mesh particle's world matrix: uniform `size`, tumbled `angle` about `axis`.
pub(crate) fn mesh_particle_matrix(position: Vec3, size: f32, axis: Vec3, angle: f32) -> Mat4 {
    let axis = axis.try_normalize().unwrap_or(Vec3::Y);
    Mat4::from_scale_rotation_translation(
        Vec3::splat(size),
        Quat::from_axis_angle(axis, angle),
        position,
    )
}

impl Renderer {
    /// Add every visible mesh emitter's particles to the camera's solid lists: opaque
    /// casings batch with the opaque list, glass shards sort with the translucent one.
    /// Returns how many were added (the frame counters' particles, not entities).
    pub(crate) fn push_mesh_particles(
        &mut self,
        scene: &Scene,
        cam: &Camera,
        lists: SolidLists,
    ) -> u32 {
        let (opaque, transparent) = lists;
        let before = opaque.len() + transparent.len();
        for id in scene.world.ids_with_particles() {
            if !scene.world.is_active(id)
                || !crate::scene::layer_in_mask(scene.world.layer(id), cam.culling_mask)
            {
                continue;
            }
            let emitter = scene.world.particles(id).expect("id has particles");
            if emitter.render.mode != ParticleRenderMode::Mesh
                || emitter.runtime.particles.is_empty()
            {
                continue;
            }
            let Some((key, glass)) = self.mesh_emitter_key(scene, &emitter) else {
                continue;
            };
            for p in &emitter.runtime.particles {
                let model =
                    mesh_particle_matrix(p.position, emitter.size_of(p), p.axis, p.rotation);
                let (use_sh, sh) = probe_sh_at(scene, p.position);
                let item = DrawItem {
                    key: key.clone(),
                    instance: InstanceData {
                        model_matrix: model.to_cols_array(),
                        use_sh,
                        sh,
                        // Picked per particle like any object (#1206).
                        reflection_probe: self.reflection_probes.probe_for(p.position),
                        ..InstanceData::IDENTITY
                    },
                    outline: false,
                };
                if glass {
                    transparent.push((item, view_depth(p.position, cam.position, cam.forward())));
                } else {
                    opaque.push(item);
                }
            }
        }
        (opaque.len() + transparent.len() - before) as u32
    }

    /// What every particle of a mesh emitter shares: the batch key (pipeline — the
    /// material's surface shader, #396 — mesh and its whole index run, material bind
    /// group, per-draw uniform) and whether the material is translucent. `None` when
    /// the mesh is unset or fails to load.
    fn mesh_emitter_key(
        &mut self,
        scene: &Scene,
        emitter: &ParticleEmitterComponent,
    ) -> Option<(BatchKey, bool)> {
        let (mesh, num_indices) = self.particle_mesh(emitter)?;
        let entry = emitter
            .render
            .material
            .as_ref()
            .and_then(|m| scene.materials.get_key_value(m))
            .map(|(key, asset)| (key.as_str(), asset));
        let material = entry.map(|(_, asset)| asset);
        for (path, kind) in material_maps(material) {
            self.load_texture_as(&path, kind);
        }
        let shader = material.and_then(|m| m.shader.as_deref());
        let pipeline = self.surface_shaders.pipeline_id(&self.device, shader);
        // Debris flies through decal boxes; it never takes their stamp (#638).
        let uniform = EntityUniform {
            receive_decals: 0,
            ..material_uniform(true, material)
        };
        let key = BatchKey {
            pipeline,
            mesh,
            // The emitter names one material: the whole mesh is one run.
            indices: (0..num_indices).into(),
            material: self.material_index(entry, pipeline, None),
            uniform: uniform.words(),
        };
        let glass = material.is_some_and(MaterialAsset::is_transparent);
        Some((key, glass))
    }
}
