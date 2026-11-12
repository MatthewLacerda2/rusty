//! src/scene/runtime/prepare.rs — the file-only half of a scene load (#2123).
//!
//! A load is two stages with a plain value between them. **Prepare** reads
//! everything a scene needs from its files alone, never touching a live scene: the
//! parsed document with every mesh rebuilt from its reference (`RehydratedScene`),
//! and the sidecars beside it (the saved navmesh, the lighting SH and the baked PVS),
//! read and decoded. The result, a [`PreparedScene`], is a pure function of the
//! files. **Activate** is [`Scene::swap_in_prepared`](crate::scene::Scene::swap_in_prepared): the swap at the tick's tail,
//! which needs the survivors and so the live scene. `Scene.Load` runs both at once.
//!
//! What prepare does not yet cover: the physics shapes (`PhysicsWorld::from_scene`
//! builds from a live scene) and the navmesh restore (`navigation`), which run at
//! activation, and model imports, which go through `asset::import_cache`'s
//! thread-local cache and so must stay on the thread that activates.

use crate::scene::lighting::io::{read_lighting_sidecar, LightingFile};
use crate::scene::nav_settings::{read_navmesh, NavmeshFile};
use crate::scene::serialize::{RehydratedScene, SceneData};

/// The sidecars beside a scene file, read and decoded, waiting to be applied.
pub struct SceneSidecars {
    pub(super) navmesh: NavmeshFile,
    pub(super) lighting: LightingFile,
}

impl SceneSidecars {
    /// Read every sidecar beside `path`.
    pub fn read(path: &str) -> Self {
        Self {
            navmesh: read_navmesh(path),
            lighting: read_lighting_sidecar(path),
        }
    }
}

/// A scene made ready from its files alone, for [`Scene::swap_in_prepared`].
///
/// [`Scene::swap_in_prepared`]: crate::scene::Scene::swap_in_prepared
pub struct PreparedScene {
    pub(super) path: String,
    pub(super) document: RehydratedScene,
    pub(super) sidecars: SceneSidecars,
}

impl PreparedScene {
    /// Put the prepared parts together: the document read from `path`, rehydrated,
    /// and the sidecars read beside it.
    pub fn new(path: String, document: RehydratedScene, sidecars: SceneSidecars) -> Self {
        Self {
            path,
            document,
            sidecars,
        }
    }

    /// Prepare `data`, the document already read from `path`, all at once.
    pub fn from_data(data: SceneData, path: &str) -> Self {
        let document = RehydratedScene::new(data);
        Self::new(path.to_string(), document, SceneSidecars::read(path))
    }

    /// The scene file this was prepared from.
    pub fn path(&self) -> &str {
        &self.path
    }
}
