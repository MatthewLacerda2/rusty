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
//! Prepare runs on the thread that ticks for a `Scene.Load`, and on a worker for a
//! `Scene.LoadAsync` (`app::scene_load::background`), which also bakes the navmesh
//! when none is saved ([`PreparedScene::set_baked_navmesh`]): it needs a scene to bake
//! from, so it bakes a [`staged`] copy that no survivor has joined. What stays at
//! activation: the physics world and the navmesh restore, both built from the live
//! scene, survivors included.

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

    /// Whether a navmesh is saved beside the scene file.
    pub fn has_navmesh(&self) -> bool {
        self.navmesh.has_bytes()
    }
}

/// A scene made ready from its files alone, for [`Scene::swap_in_prepared`].
///
/// [`Scene::swap_in_prepared`]: crate::scene::Scene::swap_in_prepared
pub struct PreparedScene {
    pub(super) path: String,
    pub(super) document: RehydratedScene,
    pub(super) sidecars: SceneSidecars,
    /// A navmesh baked from the files when none is saved beside them: restored at
    /// activation where it still matches the live scene (`<scene>.navmesh`'s bytes).
    pub(super) baked_navmesh: Option<Vec<u8>>,
}

impl PreparedScene {
    /// Put the prepared parts together: the document read from `path`, rehydrated,
    /// and the sidecars read beside it.
    pub fn new(path: String, document: RehydratedScene, sidecars: SceneSidecars) -> Self {
        Self {
            path,
            document,
            sidecars,
            baked_navmesh: None,
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

    /// The document with every mesh rebuilt, as activation will apply it.
    pub fn data(&self) -> &SceneData {
        self.document.data()
    }

    /// Keep `bytes`, a navmesh baked from these files, for activation.
    pub fn set_baked_navmesh(&mut self, bytes: Vec<u8>) {
        self.baked_navmesh = Some(bytes);
    }

    /// Take the navmesh baked from these files, if one was.
    pub fn take_baked_navmesh(&mut self) -> Option<Vec<u8>> {
        self.baked_navmesh.take()
    }
}

/// A scene holding `document` alone, as a load with no survivors would leave it,
/// sidecars aside: what a bake from the files reads.
pub fn staged(document: RehydratedScene) -> crate::scene::Scene {
    let mut scene = crate::scene::Scene::new();
    crate::scene::serialize::apply_rehydrated(&mut scene, document);
    scene
}
