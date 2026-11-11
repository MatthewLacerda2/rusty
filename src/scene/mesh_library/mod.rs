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
//! Revisions only move forward: an entry put back (an Undo, Stop's snapshot
//! restore, [`put_back`]) takes a fresh revision, so the renderer, which keeps
//! the newest upload for a name, never keeps the newer buffers.
//!
//! In Play an entry made or replaced is Play state: Stop's snapshot puts the edit
//! library back. A prefab never holds one (`prefab::write_prefab_file` refuses).
//!
//! An entity **uses** an entry through its Mesh or a Mesh collider's kept copy
//! (#1657); [`scene_mesh_users`] is the one test of that. A library read from a
//! file is held to `Mesh.Create`'s rules ([`checked`]), and an entry is deleted by
//! `delete` (`Mesh.DeleteAsset`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::components::mesh::{BoneBinding, MeshData, MeshRevision, Vertex, SCENE_KIND};
use crate::components::{ColliderComponent, MeshComponent};
use crate::scene::authoring::{primitive_mesh_component, ComponentKind, Primitive};
use crate::scene::Scene;

/// What an entry draws: its vertices and indices, built from its data.
pub type Geometry = (Vec<Vertex>, Vec<u32>);

/// One library entry: the data as authored, shared (a Play snapshot or an Undo
/// baseline clones the library, never the vertices), the geometry built from it
/// once (on first use, so every entity drawing it copies one build), and the
/// revision every entity drawing it carries. Two entries are equal when their data
/// is: the revision is not an edit (Undo compares the library). Saved as its data
/// alone.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(from = "MeshData", into = "MeshData")]
pub struct SceneMesh {
    pub data: Arc<MeshData>,
    pub revision: MeshRevision,
    built: Arc<OnceLock<Geometry>>,
}

impl SceneMesh {
    /// The geometry its data builds (`MeshData::geometry`), built on the first call.
    /// Call it on an entry that passed `MeshData::validate`, as every entry in a
    /// scene's library has.
    pub fn geometry(&self) -> &Geometry {
        self.built.get_or_init(|| {
            #[cfg(test)]
            BUILDS.with(|n| n.set(n.get() + 1));
            self.data.geometry()
        })
    }
}

#[cfg(test)]
thread_local! {
    /// How many geometries [`SceneMesh::geometry`] built on this thread.
    pub(crate) static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl PartialEq for SceneMesh {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data) || self.data == other.data
    }
}

impl From<MeshData> for SceneMesh {
    fn from(data: MeshData) -> Self {
        Self {
            data: Arc::new(data),
            revision: MeshRevision::next(),
            built: Arc::default(),
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

/// The library entry a Mesh collider's kept copy (#1657) names, when it names one.
pub fn kept_mesh_name(collider: &ColliderComponent) -> Option<&str> {
    let kept = collider.shared_mesh.as_ref()?;
    (kept.primitive_type == SCENE_KIND)
        .then_some(kept.asset_ref.as_deref())
        .flatten()
}

/// Every library entry an entity uses: the one its Mesh draws, then the one its
/// Mesh collider's kept copy names. The one test of "uses a scene mesh" that the
/// sync, a Play load's carried survivors, a prefab save and `Mesh.DeleteAsset`
/// share.
pub fn scene_mesh_users<'a>(
    mesh: Option<&'a MeshComponent>,
    collider: Option<&'a ColliderComponent>,
) -> impl Iterator<Item = &'a str> {
    let own = mesh.and_then(scene_mesh_name);
    own.into_iter().chain(collider.and_then(kept_mesh_name))
}

/// `library` as a file brought it, each entry held to what `Mesh.Create` holds
/// (its name and `MeshData::validate`): a bad entry is dropped with a warning
/// naming it, and the entities drawing it draw nothing, as for a missing model.
pub fn checked(library: MeshLibrary) -> MeshLibrary {
    let mut kept = MeshLibrary::new();
    for (name, mesh) in library {
        match check_name(&name).and_then(|()| mesh.data.validate()) {
            Ok(()) => {
                kept.insert(name, mesh);
            }
            Err(why) => crate::scene::rehydrate::warn_once_of(&missing_key(&name), || {
                format!("the scene's mesh {name:?} is dropped: {why}")
            }),
        }
    }
    kept
}

/// The once-only warning key of a library mesh `name` an entity cannot draw: one
/// key whether [`checked`] dropped it or no file ever defined it, so a dropped
/// entry is warned of once, with its cause, and not again as missing.
pub(crate) fn missing_key(name: &str) -> String {
    format!("{SCENE_KIND}::{name}")
}

/// Put `library` back in `slot` (Undo's put-back, Stop's snapshot restore) with
/// revisions that only move forward: an entry whose data is the live entry's
/// (the same `Arc`) keeps the live revision, every other one takes a fresh one.
/// A put-back entry's revision is older than what the renderer may hold for its
/// name (an entity drew a newer replace since), and the renderer keeps the
/// newest upload, so an old revision would draw the newer data.
pub fn put_back(mut library: MeshLibrary, slot: &mut MeshLibrary) {
    for (name, entry) in &mut library {
        entry.revision = match slot.get(name) {
            Some(live) if Arc::ptr_eq(&live.data, &entry.data) => live.revision,
            _ => MeshRevision::next(),
        };
    }
    *slot = library;
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
            // The entry's own build and revision: the other entities drawing it
            // stay as they are, and nothing is built again.
            None if self.meshes.contains_key(reference) => {
                let entry = &self.meshes[reference];
                let (vertices, indices) = entry.geometry().clone();
                MeshComponent {
                    asset_ref: Some(reference.to_string()),
                    revision: entry.revision,
                    ..MeshComponent::new(SCENE_KIND, vertices, indices)
                }
            }
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

mod delete;
mod sync;

#[cfg(test)]
mod delete_tests;
#[cfg(test)]
mod revision_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod users_tests;
