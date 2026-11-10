//! src/scene/nav_settings/saved.rs — the `<scene>.navmesh` file (#1887): the saved
//! navmesh beside the scene, Unity's baked `NavMeshData`.
//!
//! The scene only carries the file's bytes ([`SavedNavmesh`]): what they mean, and
//! whether they still match the scene, is `navigation`'s (the arrow is navigation →
//! scene). A save writes the bytes the scene holds to its own path, so *Save As*
//! carries the navmesh to the new path as it carries the baked PVS; a scene that holds
//! none removes only a file it read or wrote itself (the file is checked on use, so
//! a stale one costs only a full bake). A save never bakes: `Navigation.Bake()` and the
//! editor's *Bake Navigation* button do. Nor does it rewrite bytes it only loaded
//! (`scene::io::BakeFiles`, #1982): a Dust2-sized navmesh is tens of MB, most saves
//! leave it as it was loaded, and a newer one another process baked is kept.
//!
//! stored-format: the `<scene>.navmesh` file beside the scene

use std::path::PathBuf;
use std::sync::Arc;

use crate::scene::io::Saved;
use crate::scene::Scene;

const SUFFIX: &str = ".navmesh";

/// The scene's saved navmesh: its bytes. What the engine last read or wrote at
/// which path is the scene's [`BakeFiles`](crate::scene::io::BakeFiles) record.
#[derive(Clone, Default)]
pub struct SavedNavmesh {
    /// The `<scene>.navmesh` bytes, checked on use (`navigation`); `None` for none.
    pub bytes: Option<Arc<[u8]>>,
}

impl SavedNavmesh {
    /// Hold `bytes`.
    pub fn new(bytes: Option<Arc<[u8]>>) -> Self {
        Self { bytes }
    }
}

/// Where `scene_path`'s navmesh is kept: `foo.scene` → `foo.scene.navmesh`.
pub fn navmesh_path(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{SUFFIX}"))
}

/// Bring the navmesh file beside `scene_path` to what `scene` holds, by the bake
/// files' rule (`scene::io::BakeFiles`, #1982): written only when this world holds
/// bytes it did not last read or write there, and never over a newer file with
/// bytes it only loaded.
pub fn save_navmesh(scene: &Scene, scene_path: &str) -> Result<Saved, String> {
    let path = navmesh_path(scene_path);
    let want = scene.navmesh.bytes.as_deref();
    scene.bake_files.store(&path, want, "Navigation")
}

/// Read the navmesh beside `scene_path` onto `scene`: none when there is no file,
/// and none, with a warning, when it cannot be read. Its content is checked when Play
/// uses it (`navigation`), never here.
pub fn load_navmesh(scene: &mut Scene, scene_path: &str) {
    let path = navmesh_path(scene_path);
    let bytes: Option<Arc<[u8]>> = match std::fs::read(&path) {
        Ok(bytes) => Some(Arc::from(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            log::warn!(
                "[Navigation] could not read {} ({e}): Play bakes the navmesh in full",
                path.display()
            );
            scene.bake_files.unreadable(&path);
            scene.navmesh = SavedNavmesh::default();
            return;
        }
    };
    scene
        .bake_files
        .loaded(&path, bytes.as_deref(), bytes.as_deref());
    scene.navmesh = SavedNavmesh { bytes };
}

impl Scene {
    /// Put a navmesh bake's bytes on the scene: the one place a bake's output
    /// lands, so its next save writes them, even over a newer file another process
    /// baked (#1982).
    pub fn put_baked_navmesh(&mut self, bytes: Arc<[u8]>) {
        self.bake_files.baked(SUFFIX);
        self.navmesh = SavedNavmesh::new(Some(bytes));
    }
}

#[cfg(test)]
#[path = "saved_tests.rs"]
mod tests;
