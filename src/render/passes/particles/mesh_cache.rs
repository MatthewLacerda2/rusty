//! src/render/passes/particles/mesh_cache.rs — which GPU mesh each mesh emitter's
//! spec (`"Box"`, `"models/casing.fbx::Casing"`) resolved to, kept in step with
//! the files on disk (#1207, #1219).
//!
//! A spec is read from disk once, not every frame, and two things send it back:
//!
//! - **The engine re-wrote its model.** An import notes the source path in
//!   `core::fs::written` once its conversion is swapped in, and an `.obj` or glTF
//!   the engine writes is noted by its writer. The renderer drains the notes each
//!   frame (`refresh_textures` → `invalidate_textures`), and every spec whose file
//!   one names is marked [`CachedMesh::Stale`]: the next draw re-reads it and
//!   overwrites the resident upload, which an entity carrying the same model may
//!   share.
//! - **It failed to load.** A failed load is a [`CachedMesh::Missing`] miss, not a
//!   cached answer: misses are dropped every `MISS_RETRY_FRAMES` frames, with the
//!   texture cache's, so a model imported after the first draw is picked up.

use crate::components::ParticleEmitterComponent;
use crate::core::fs::written::Stale;
use crate::render::{MeshId, Renderer};
use crate::scene::serialize::asset_ref_file;

/// What one mesh-emitter spec resolved to.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CachedMesh {
    /// Resident on the GPU under this id.
    Loaded(MeshId),
    /// Failed to load: skipped until the next retry.
    Missing,
    /// Its file was re-written: read again, replacing the resident upload.
    Stale,
}

impl Renderer {
    /// The emitter's mesh, resident on the GPU: `(id, index count)`. Loaded once per
    /// spec and remembered; a spec that fails to load is skipped until the next
    /// retry instead of re-read from disk every frame.
    pub(super) fn particle_mesh(
        &mut self,
        emitter: &ParticleEmitterComponent,
    ) -> Option<(MeshId, u32)> {
        let spec = emitter.render.mesh.as_deref()?;
        let cached = self.particle_renderer.meshes.get(spec).cloned();
        let id = match cached {
            Some(CachedMesh::Missing) => return None,
            Some(CachedMesh::Loaded(id)) if self.gpu_meshes.contains_key(&id) => id,
            _ => {
                let mesh = super::mesh::mesh_for_spec(spec);
                let id = MeshId::from_mesh(&mesh);
                // An entity may already have it resident (the same model or
                // primitive); a stale spec replaces that upload with the new file.
                if cached == Some(CachedMesh::Stale) || !self.gpu_meshes.contains_key(&id) {
                    self.update_gpu_mesh(id.clone(), &mesh.vertices, &mesh.indices);
                }
                let loaded = !mesh.indices.is_empty() && self.gpu_meshes.contains_key(&id);
                let entry = match loaded {
                    true => CachedMesh::Loaded(id.clone()),
                    false => CachedMesh::Missing,
                };
                self.particle_renderer
                    .meshes
                    .insert(spec.to_string(), entry);
                loaded.then_some(id)?
            }
        };
        let num_indices = self.gpu_meshes.get(&id)?.num_indices;
        Some((id, num_indices))
    }

    /// Mark every spec whose model file `stale` names, so its next draw reads it
    /// again.
    pub(crate) fn forget_particle_meshes(&mut self, stale: &Stale) {
        for (spec, cached) in &mut self.particle_renderer.meshes {
            if asset_ref_file(spec).is_some_and(|file| stale.matches(&file)) {
                *cached = CachedMesh::Stale;
            }
        }
    }

    /// Drop every miss, so each spec that failed to load is tried once more.
    pub(crate) fn retry_particle_meshes(&mut self) {
        self.particle_renderer
            .meshes
            .retain(|_, cached| *cached != CachedMesh::Missing);
    }
}

#[cfg(test)]
#[path = "mesh_cache_tests.rs"]
mod tests;
