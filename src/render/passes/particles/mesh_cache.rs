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
//!   one names is marked [`CachedMesh::Stale`] ([`mark_stale`]): the next draw
//!   re-reads it and overwrites the resident upload, which an entity carrying the
//!   same model may share. A note the spec's own load made (an emitter whose first
//!   draw ran the import) is older than the load, and is ignored.
//! - **Its file appeared.** A failed load is a [`CachedMesh::Missing`] miss, not a
//!   cached answer. Every `MISS_RETRY_FRAMES` frames, with the texture cache's
//!   misses, a miss whose file was absent and now exists is dropped
//!   ([`retry_appeared`]), so a model copied in after the first draw is picked up.
//!   A file that was there and failed (a broken FBX) is not re-read on a timer,
//!   which would re-run its conversion once a second: only a note sends it back.

use std::collections::HashMap;
use std::path::Path;

use crate::components::ParticleEmitterComponent;
use crate::core::fs::written::{written_since, Stale};
use crate::render::{MeshId, Renderer};
use crate::scene::serialize::asset_ref_file;

/// What one mesh-emitter spec resolved to.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CachedMesh {
    /// Resident on the GPU under `id`, loaded when the write log stood at
    /// generation `at`.
    Loaded { id: MeshId, at: u64 },
    /// Failed to load; `existed` says whether its file was there when it did.
    Missing { existed: bool },
    /// Its file was re-written: read again, replacing the resident upload.
    Stale,
}

/// Whether `spec` names a model file that is on disk now.
fn file_exists(spec: &str) -> bool {
    asset_ref_file(spec).is_some_and(|file| Path::new(&file).exists())
}

/// Mark `Stale` every spec whose model file `stale` names, unless every note of
/// it is one its own load made.
pub(super) fn mark_stale(meshes: &mut HashMap<String, CachedMesh>, stale: &Stale) {
    for (spec, cached) in meshes.iter_mut() {
        let Some(file) = asset_ref_file(spec).filter(|file| stale.matches(file)) else {
            continue;
        };
        if let CachedMesh::Loaded { at, .. } = cached {
            if !Stale::new(written_since(*at).1).matches(&file) {
                continue;
            }
        }
        *cached = CachedMesh::Stale;
    }
}

/// Drop every miss whose file was absent and has appeared, so it is tried once
/// more. A spec naming no file (a primitive) is always tried again: no disk read.
pub(super) fn retry_appeared(meshes: &mut HashMap<String, CachedMesh>) {
    meshes.retain(|spec, cached| match cached {
        CachedMesh::Missing { existed: false } => {
            asset_ref_file(spec).is_some_and(|file| !Path::new(&file).exists())
        }
        _ => true,
    });
}

impl Renderer {
    /// The emitter's mesh, resident on the GPU: `(id, index count)`. Loaded once per
    /// spec and remembered; a spec that fails to load is skipped until its file
    /// appears or is re-written, instead of re-read from disk every frame.
    pub(super) fn particle_mesh(
        &mut self,
        emitter: &ParticleEmitterComponent,
    ) -> Option<(MeshId, u32)> {
        let spec = emitter.render.mesh.as_deref()?;
        let cached = self.particle_renderer.meshes.get(spec).cloned();
        let id = match cached {
            Some(CachedMesh::Missing { .. }) => return None,
            Some(CachedMesh::Loaded { id, .. }) if self.gpu_meshes.contains_key(&id) => id,
            _ => {
                let mesh = super::mesh::mesh_for_spec(spec);
                let id = MeshId::from_mesh(&mesh);
                // An entity may already have it resident (the same model or
                // primitive); a stale spec replaces that upload with the new file.
                if cached == Some(CachedMesh::Stale) || !self.gpu_meshes.contains_key(&id) {
                    self.update_gpu_mesh(id.clone(), mesh.revision, &mesh.vertices, &mesh.indices);
                }
                let loaded = !mesh.indices.is_empty() && self.gpu_meshes.contains_key(&id);
                let entry = match loaded {
                    true => CachedMesh::Loaded {
                        id: id.clone(),
                        at: written_since(u64::MAX).0,
                    },
                    false => CachedMesh::Missing {
                        existed: file_exists(spec),
                    },
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
    /// again ([`mark_stale`]).
    pub(crate) fn forget_particle_meshes(&mut self, stale: &Stale) {
        mark_stale(&mut self.particle_renderer.meshes, stale);
    }

    /// Drop every miss whose file has appeared ([`retry_appeared`]).
    pub(crate) fn retry_particle_meshes(&mut self) {
        retry_appeared(&mut self.particle_renderer.meshes);
    }
}

#[cfg(test)]
#[path = "mesh_cache_tests.rs"]
mod tests;
