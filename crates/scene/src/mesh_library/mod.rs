use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::create::Primitive;
use crate::Scene;
use rusty_asset::mesh_data::Shared;
use rusty_components::mesh::{MeshData, MeshRevision, Vertex, SCENE_KIND};
use rusty_components::{ColliderComponent, MeshComponent};

pub type Geometry = (Shared<Vertex>, Shared<u32>);

/// Equal by data alone: the revision is not an edit (Undo compares the library).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(from = "MeshData", into = "MeshData")]
pub struct SceneMesh {
    pub data: Arc<MeshData>,
    pub revision: MeshRevision,
    built: Arc<OnceLock<Geometry>>,
}

impl SceneMesh {
    /// Only for an entry that passed `MeshData::validate`.
    pub fn geometry(&self) -> &Geometry {
        self.built.get_or_init(|| {
            BUILDS.with(|n| n.set(n.get() + 1));
            let (vertices, indices) = self.data.geometry();
            (vertices.into(), indices.into())
        })
    }

    pub fn indices(&self) -> Shared<u32> {
        match self.built.get() {
            Some((_, indices)) => indices.clone(),
            None => self.data.indices.clone().into(),
        }
    }
}

thread_local! {
    /// A test hook: geometries built on this thread, read by the edit side's tests.
    #[doc(hidden)]
    pub static BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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

pub type MeshLibrary = BTreeMap<String, SceneMesh>;

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

pub fn scene_mesh_name(mesh: &MeshComponent) -> Option<&str> {
    (mesh.primitive_type == SCENE_KIND)
        .then_some(mesh.asset_ref.as_deref())
        .flatten()
}

pub fn kept_mesh_name(collider: &ColliderComponent) -> Option<&str> {
    let kept = collider.shared_mesh.as_ref()?;
    (kept.primitive_type == SCENE_KIND)
        .then_some(kept.asset_ref.as_deref())
        .flatten()
}

pub fn scene_mesh_users<'a>(
    mesh: Option<&'a MeshComponent>,
    collider: Option<&'a ColliderComponent>,
) -> impl Iterator<Item = &'a str> {
    let own = mesh.and_then(scene_mesh_name);
    own.into_iter().chain(collider.and_then(kept_mesh_name))
}

pub fn checked(library: MeshLibrary) -> MeshLibrary {
    let mut kept = MeshLibrary::new();
    for (name, mesh) in library {
        match check_name(&name).and_then(|()| mesh.data.validate()) {
            Ok(()) => {
                kept.insert(name, mesh);
            }
            Err(why) => crate::rehydrate::warn_once_of(&missing_key(&name), || {
                format!("the scene's mesh {name:?} is dropped: {why}")
            }),
        }
    }
    kept
}

/// One key whether `checked` dropped the entry or no file defined it, so a dropped
/// entry is warned of once, with its cause.
pub fn missing_key(name: &str) -> String {
    format!("{SCENE_KIND}::{name}")
}

/// Revisions only move forward: the renderer keeps a name's newest upload, so an older
/// revision put back would draw the newer data. Entities catch up only at the next
/// `Scene::sync_scene_meshes`, so run it once they are back.
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
    pub fn define_mesh(&mut self, name: &str, data: MeshData) -> Result<(), String> {
        check_name(name)?;
        data.validate()?;
        self.meshes.insert(name.to_string(), data.into());
        self.sync_scene_meshes();
        Ok(())
    }

    pub fn sync_scene_meshes(&mut self) {
        let changed: BTreeSet<String> = sync::stale_entries(self);
        sync::install(self, &changed);
    }
}

mod delete;
mod share;
mod sync;

pub use share::{put_shared, resolve_shared};
pub use sync::refit;
