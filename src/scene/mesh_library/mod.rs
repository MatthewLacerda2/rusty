//! src/scene/mesh_library/ — the scene's meshes made from data (#2059): Unity's
//! `Mesh` built in a script and put on a `MeshFilter`.
//!
//! The library sits beside the material library (`Scene::materials`): named
//! entries saved with the scene (`SceneData::meshes`), each a [`MeshData`]. An
//! entity draws one when its Mesh's `primitive_type` is [`SCENE_KIND`] and its
//! `asset_ref` is the entry's name. Its geometry is a copy of the entry's, so
//! everything that reads a mesh (the renderer, physics, the bakes) reads it as it
//! reads a primitive's.
//!
//! **One revision per entry.** Every entity drawing an entry carries the entry's
//! [`MeshRevision`], and the renderer keys the GPU buffers by the reference
//! (`render::MeshId`), so all of them share one buffer pair, uploaded once per
//! change. [`Scene::sync_scene_meshes`] brings every entity to its entry's
//! revision: after `Mesh.Create`, a load, a Play snapshot's put-back and an Undo.
//! An entry going back to older data (an Undo) takes a fresh revision first, so
//! the renderer never keeps the newer buffers.
//!
//! In Play an entry made or replaced is Play state: Stop's snapshot puts the edit
//! library back. A prefab never holds one (`prefab::write_prefab_file` refuses).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::components::mesh::{BoneBinding, MeshData, MeshRevision, SCENE_KIND};
use crate::components::MeshComponent;
use crate::scene::authoring::{primitive_mesh_component, ComponentKind, Primitive};
use crate::scene::Scene;

/// One library entry: the data as authored, shared (a Play snapshot or an Undo
/// baseline clones the library, never the vertices), and the revision every entity
/// drawing it carries. Two entries are equal when their data is: the revision is
/// not an edit (Undo compares the library). Saved as its data alone.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(from = "MeshData", into = "MeshData")]
pub struct SceneMesh {
    pub data: Arc<MeshData>,
    pub revision: MeshRevision,
}

impl PartialEq for SceneMesh {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) || self.data == other.data
    }
}

impl From<MeshData> for SceneMesh {
    fn from(data: MeshData) -> Self {
        let revision = MeshRevision::next();
        Self {
            data: Arc::new(data),
            revision,
        }
    }
}

impl From<SceneMesh> for MeshData {
    fn from(mesh: SceneMesh) -> Self {
        Arc::unwrap_or_clone(mesh.data)
    }
}

/// The library: entries by name, in a `BTreeMap` so a save writes them in order.
pub type MeshLibrary = BTreeMap<String, SceneMesh>;

/// Why `name` cannot name a library mesh: empty, a primitive's name (which
/// `Mesh.SetSharedMesh` reads as the primitive), or holding `::` (a model
/// reference's separator).
pub fn check_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("a mesh needs a name".into());
    }
    if Primitive::parse(name).is_some() {
        return Err(format!("{name:?} is a primitive's name; pick another"));
    }
    if name.contains("::") {
        return Err(format!(
            "{name:?} holds '::', a model reference's separator"
        ));
    }
    Ok(())
}

/// The library entry `mesh` draws, by name, when it draws one.
pub fn scene_mesh_name(mesh: &MeshComponent) -> Option<&str> {
    (mesh.primitive_type == SCENE_KIND)
        .then_some(mesh.asset_ref.as_deref())
        .flatten()
}

impl Scene {
    /// Define (or replace) the library mesh `name` from `data`, and bring every
    /// entity drawing it up to date: Unity's shared mesh, edited in place.
    pub fn define_mesh(&mut self, name: &str, data: MeshData) -> Result<(), String> {
        check_name(name)?;
        data.validate()?;
        self.meshes.insert(name.to_string(), data.into());
        self.sync_scene_meshes();
        Ok(())
    }

    /// Point entity `id`'s Mesh at `reference`: a primitive's name (`"Box"`, …) or a
    /// library mesh's, adding a Mesh when it has none (Unity's
    /// `MeshFilter.sharedMesh`). The Mesh's own settings (shadows, culling, Scale In
    /// Lightmap) stay; its geometry, rig and grid are the new mesh's.
    pub fn set_shared_mesh(&mut self, id: u32, reference: &str) -> Result<(), String> {
        if !self.world.contains(id) {
            return Err(format!("no entity {id}"));
        }
        let fresh = match Primitive::parse(reference) {
            Some(p) => primitive_mesh_component(p)
                .ok_or_else(|| format!("{reference:?} is a light, not a mesh"))?,
            None if self.meshes.contains_key(reference) => MeshComponent {
                asset_ref: Some(reference.to_string()),
                ..MeshComponent::new(SCENE_KIND, Vec::new(), Vec::new())
            },
            None => {
                return Err(format!(
                    "no mesh named {reference:?} (a primitive, or one made with Mesh.Create)"
                ))
            }
        };
        if self.world.mesh(id).is_none() {
            crate::scene::authoring::add_component(self, id, ComponentKind::Mesh);
        }
        if let Some(mut mesh) = self.world.mesh_mut(id) {
            *mesh = MeshComponent {
                cast_shadows: mesh.cast_shadows,
                scale_in_lightmap: mesh.scale_in_lightmap,
                max_draw_distance: mesh.max_draw_distance,
                min_screen_size: mesh.min_screen_size,
                skeleton: BoneBinding::default(),
                ..fresh
            };
        }
        self.sync_scene_meshes();
        sync::refit(self, id);
        Ok(())
    }

    /// Bring every entity drawing a library mesh to its entry's geometry and
    /// revision, and fill a Mesh collider's kept copy of one (#1657). Cheap when
    /// nothing changed: one comparison per Mesh entity.
    pub fn sync_scene_meshes(&mut self) {
        let changed: BTreeSet<String> = sync::stale_entries(self);
        sync::install(self, &changed);
    }
}

mod sync;

#[cfg(test)]
mod tests;
