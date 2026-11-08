//! src/scene/nav_settings/saved.rs — the `<scene>.navmesh` file (#1887): the saved
//! navmesh beside the scene, Unity's baked `NavMeshData`.
//!
//! The scene only carries the file's bytes ([`SavedNavmesh`]): what they mean, and
//! whether they still match the scene, is `navigation`'s (the arrow is navigation →
//! scene). A save writes the bytes the scene holds to its own path, so *Save As*
//! carries the navmesh to the new path as it carries the baked PVS; a scene that holds
//! none leaves any file at its path alone (the file is checked on use, so a stale one
//! costs only a full bake). A save never bakes: `Navigation.Bake()` and the editor's
//! *Bake Navigation* button do. Nor does it rewrite bytes the file already holds: a
//! Dust2-sized navmesh is tens of MB, and most saves leave it as it was loaded.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use crate::scene::Scene;

const SUFFIX: &str = ".navmesh";

/// The scene's saved navmesh: its bytes, and what the engine last read or wrote at
/// which path, so a save skips a file that already holds them.
#[derive(Clone, Default)]
pub struct SavedNavmesh {
    /// The `<scene>.navmesh` bytes, checked on use (`navigation`); `None` for none.
    pub bytes: Option<Arc<[u8]>>,
    /// Interior mutability because a save takes `&Scene`, as `DiskText` does.
    on_disk: RefCell<Option<(PathBuf, Arc<[u8]>)>>,
}

impl SavedNavmesh {
    /// Hold `bytes`.
    pub fn new(bytes: Option<Arc<[u8]>>) -> Self {
        Self {
            bytes,
            on_disk: RefCell::default(),
        }
    }

    /// Whether the file at `path` holds `bytes`, as last read or written there.
    fn on_disk(&self, path: &PathBuf, bytes: &Arc<[u8]>) -> bool {
        let last = self.on_disk.borrow();
        let same = last
            .as_ref()
            .is_some_and(|(p, b)| p == path && (Arc::ptr_eq(b, bytes) || **b == **bytes));
        // A file deleted or resized outside the engine since is written again.
        same && std::fs::metadata(path).is_ok_and(|m| m.len() == bytes.len() as u64)
    }
}

/// Where `scene_path`'s navmesh is kept: `foo.scene` → `foo.scene.navmesh`.
pub fn navmesh_path(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{SUFFIX}"))
}

/// Write the navmesh `scene` holds beside `scene_path`: nothing when it holds none,
/// or when the file already holds those bytes. Returns whether it wrote.
pub fn save_navmesh(scene: &Scene, scene_path: &str) -> Result<bool, String> {
    let saved = &scene.navmesh;
    let Some(bytes) = saved.bytes.as_ref() else {
        return Ok(false);
    };
    let path = navmesh_path(scene_path);
    if saved.on_disk(&path, bytes) {
        return Ok(false);
    }
    // lint: allow(asset-write) the saved navmesh, a bake document read whole on load
    std::fs::write(&path, bytes).map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    *saved.on_disk.borrow_mut() = Some((path, bytes.clone()));
    Ok(true)
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
            None
        }
    };
    let on_disk = bytes.clone().map(|b| (path, b));
    scene.navmesh = SavedNavmesh {
        bytes,
        on_disk: RefCell::new(on_disk),
    };
}

#[cfg(test)]
#[path = "saved_tests.rs"]
mod tests;
