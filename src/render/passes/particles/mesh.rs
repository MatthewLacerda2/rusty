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
use crate::render::draw::batch::{BatchKey, DrawItem};
use crate::render::draw::sort::view_depth;
use crate::render::draw::uniforms::{material_uniform, probe_sh_at};
use crate::render::{EntityUniform, InstanceData, MeshId, Renderer};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::serialize::asset_mesh_component;
use crate::scene::{Camera, MeshComponent, Scene};

/// The camera's opaque items and its (unsorted) translucent items with their depth.
type SolidLists<'a> = (&'a mut Vec<DrawItem>, &'a mut Vec<(DrawItem, f32)>);

/// A mesh named by one string: a primitive (`"Box"`) or a model reference.
fn mesh_for_spec(spec: &str) -> MeshComponent {
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
            let Some((key, num_indices, glass)) = self.mesh_emitter_key(scene, &emitter) else {
                continue;
            };
            for p in &emitter.runtime.particles {
                let model =
                    mesh_particle_matrix(p.position, emitter.size_of(p), p.axis, p.rotation);
                let (use_sh, sh) = probe_sh_at(scene, p.position);
                let item = DrawItem {
                    key: key.clone(),
                    num_indices,
                    instance: InstanceData {
                        model_matrix: model.to_cols_array(),
                        use_sh,
                        _pad: [0; 3],
                        sh,
                    },
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
    /// material's surface shader, #396 — mesh, material bind group, per-draw uniform), the index count, and whether the material is
    /// translucent. `None` when the mesh is unset or fails to load.
    fn mesh_emitter_key(
        &mut self,
        scene: &Scene,
        emitter: &ParticleEmitterComponent,
    ) -> Option<(BatchKey, u32, bool)> {
        let (mesh, num_indices) = self.particle_mesh(emitter)?;
        let entry = emitter
            .render
            .material
            .as_ref()
            .and_then(|m| scene.materials.get_key_value(m))
            .map(|(key, asset)| (key.as_str(), asset));
        let material = entry.map(|(_, asset)| asset);
        for path in material_maps(material) {
            self.load_texture(&path);
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
            material: self.material_index(entry, pipeline, None),
            uniform: uniform.words(),
        };
        let glass = material.is_some_and(MaterialAsset::is_transparent);
        Some((key, num_indices, glass))
    }

    /// The emitter's mesh, resident on the GPU: `(id, index count)`. Loaded once per
    /// mesh name and remembered — including a name that fails to load, which is then
    /// skipped instead of re-read from disk every frame.
    fn particle_mesh(&mut self, emitter: &ParticleEmitterComponent) -> Option<(MeshId, u32)> {
        let spec = emitter.render.mesh.as_deref()?;
        let cached = self.particle_renderer.meshes.get(spec).cloned();
        let id = match cached {
            Some(None) => return None,
            Some(Some(id)) if self.gpu_meshes.contains_key(&id) => id,
            _ => {
                let mesh = mesh_for_spec(spec);
                let id = MeshId::from_mesh(&mesh);
                // An entity may already have it resident (the same model or primitive).
                if !self.gpu_meshes.contains_key(&id) {
                    self.update_gpu_mesh(id.clone(), &mesh.vertices, &mesh.indices);
                }
                let loaded = self.gpu_meshes.contains_key(&id).then_some(id);
                self.particle_renderer
                    .meshes
                    .insert(spec.to_string(), loaded.clone());
                loaded?
            }
        };
        let num_indices = self.gpu_meshes.get(&id)?.num_indices;
        Some((id, num_indices))
    }
}

/// The texture maps `material` samples, to upload before its bind group is built.
fn material_maps(material: Option<&MaterialAsset>) -> Vec<String> {
    crate::render::draw::materials::material_texture_paths(material)
        .into_iter()
        .flatten()
        .collect()
}
