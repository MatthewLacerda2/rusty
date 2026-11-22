//! A spec is read once, not each frame. An engine re-write (`core::fs::written`) marks it Stale;
//! a miss whose file appeared is retried every `MISS_RETRY_FRAMES`. A file that existed and failed
//! is never retried on a timer (it would re-run the conversion each second). A note made by the
//! spec's own load is ignored.

use std::collections::HashMap;
use std::path::Path;

use crate::{MeshId, Renderer};
use rusty_components::ParticleEmitterComponent;
use rusty_core::fs::written::{written_since, Stale};
use rusty_scene::serialize::asset_ref_file;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CachedMesh {
    Loaded { id: MeshId, at: u64 },
    Missing { existed: bool },
    Stale,
}

fn file_exists(spec: &str) -> bool {
    asset_ref_file(spec).is_some_and(|file| Path::new(&file).exists())
}

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

pub(super) fn retry_appeared(meshes: &mut HashMap<String, CachedMesh>) {
    meshes.retain(|spec, cached| match cached {
        CachedMesh::Missing { existed: false } => {
            asset_ref_file(spec).is_some_and(|file| !Path::new(&file).exists())
        }
        _ => true,
    });
}

impl Renderer {
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
                if cached == Some(CachedMesh::Stale) || !self.gpu_meshes.contains_key(&id) {
                    self.update_gpu_mesh(id.clone(), mesh.revision, &mesh.vertices, &mesh.indices);
                    // Scene meshes may share this upload; one without clusters would end
                    // their cluster cull.
                    self.attach_clusters(&id, (mesh.clusters(), &mesh.indices));
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

    pub(crate) fn forget_particle_meshes(&mut self, stale: &Stale) {
        mark_stale(&mut self.particle_renderer.meshes, stale);
    }

    pub(crate) fn retry_particle_meshes(&mut self) {
        retry_appeared(&mut self.particle_renderer.meshes);
    }
}

#[cfg(test)]
#[path = "mesh_cache_tests.rs"]
mod tests;
