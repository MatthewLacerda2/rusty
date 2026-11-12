//! src/scene/nav_settings/saved.rs — the `<scene>.navmesh` file (#1887): the saved
//! navmesh beside the scene, Unity's baked `NavMeshData`.
//!
//! The scene only carries the file's bytes ([`SavedNavmesh`]): what they mean, and
//! whether they still match the scene, is `navigation`'s (the arrow is navigation →
//! scene). A save writes the bytes the scene holds to its own path, so *Save As*
//! carries the navmesh to the new path as it carries the baked PVS. A scene that holds
//! none removes a file it read or wrote itself only while that file is unchanged, and
//! a save to a path it has no record for (a new scene saved over a baked one) replaces
//! whatever navmesh is there (the file is checked on use, so a stale one costs only
//! a full bake). A save never bakes: `Navigation.Bake()` and the
//! editor's *Bake Navigation* button do. Nor does it rewrite bytes it only loaded
//! (`scene::io::BakeFiles`, #1982): a Dust2-sized navmesh is tens of MB, most saves
//! leave it as it was loaded (the held buffer is not hashed again), and a newer one
//! another process baked is kept.
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
/// files' rule (`scene::io::BakeFiles`, #1982): written only when this world baked
/// since or holds bytes it did not last read or write there, and never over a newer file with
/// bytes it only loaded.
pub fn save_navmesh(scene: &Scene, scene_path: &str) -> Result<Saved, String> {
    let path = navmesh_path(scene_path);
    let want = scene.navmesh.bytes.as_ref();
    scene.bake_files.store_shared(&path, want, "Navigation")
}

/// What a load found at `<scene>.navmesh`, read from the file alone
/// ([`read_navmesh`]) and put on a scene later ([`apply_navmesh`]), so a load can
/// read it before it touches the live scene (#2123).
pub struct NavmeshFile {
    path: PathBuf,
    read: std::io::Result<Option<Arc<[u8]>>>,
}

/// Read the navmesh beside `scene_path`: none when there is no file.
pub fn read_navmesh(scene_path: &str) -> NavmeshFile {
    let path = navmesh_path(scene_path);
    let read = match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(Arc::from(bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    };
    NavmeshFile { path, read }
}

/// Put what [`read_navmesh`] found on `scene`: none, with a warning, when the file
/// could not be read. Its content is checked when Play uses it (`navigation`),
/// never here.
pub fn apply_navmesh(scene: &mut Scene, file: NavmeshFile) {
    let NavmeshFile { path, read } = file;
    let bytes = match read {
        Ok(bytes) => bytes,
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
    scene.bake_files.loaded_shared(&path, bytes.as_ref());
    scene.navmesh = SavedNavmesh { bytes };
}

/// Read the navmesh beside `scene_path` onto `scene`, at once
/// ([`read_navmesh`] then [`apply_navmesh`]).
pub fn load_navmesh(scene: &mut Scene, scene_path: &str) {
    apply_navmesh(scene, read_navmesh(scene_path));
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
