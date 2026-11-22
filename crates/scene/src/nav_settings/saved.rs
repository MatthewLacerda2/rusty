//! stored-format: the `<scene>.navmesh` file beside the scene
//!
//! The scene holds the file's bytes; `navigation` checks them on use.

use std::path::PathBuf;
use std::sync::Arc;

use crate::io::Saved;
use crate::Scene;

const SUFFIX: &str = ".navmesh";

#[derive(Clone, Default)]
pub struct SavedNavmesh {
    pub bytes: Option<Arc<[u8]>>,
}

impl SavedNavmesh {
    pub fn new(bytes: Option<Arc<[u8]>>) -> Self {
        Self { bytes }
    }
}

pub fn navmesh_path(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{SUFFIX}"))
}

pub fn save_navmesh(scene: &Scene, scene_path: &str) -> Result<Saved, String> {
    let path = navmesh_path(scene_path);
    let want = scene.navmesh.bytes.as_ref();
    scene.bake_files.store_shared(&path, want, "Navigation")
}

pub struct NavmeshFile {
    path: PathBuf,
    read: std::io::Result<Option<Arc<[u8]>>>,
}

impl NavmeshFile {
    pub fn has_bytes(&self) -> bool {
        matches!(self.read, Ok(Some(_)))
    }
}

/// Reads the file alone, so a load can read it before it touches the live scene.
pub fn read_navmesh(scene_path: &str) -> NavmeshFile {
    let path = navmesh_path(scene_path);
    let read = match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(Arc::from(bytes))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    };
    NavmeshFile { path, read }
}

pub fn apply_navmesh(scene: &mut Scene, file: NavmeshFile) {
    let NavmeshFile { path, read } = file;
    let bytes = match read {
        Ok(bytes) => bytes,
        Err(e) => {
            log::warn!(
                "[Navigation] could not read {} ({e}): Play bakes the navmesh in full",
                path.display()
            );
            scene.bake_files.unreadable(&path, &e.to_string());
            scene.navmesh = SavedNavmesh::default();
            return;
        }
    };
    scene.bake_files.loaded_shared(&path, bytes.as_ref());
    scene.navmesh = SavedNavmesh { bytes };
}

pub fn load_navmesh(scene: &mut Scene, scene_path: &str) {
    apply_navmesh(scene, read_navmesh(scene_path));
}

impl Scene {
    /// The one place a bake's output lands, so the next save writes it even over a
    /// newer file another process baked.
    pub fn put_baked_navmesh(&mut self, bytes: Arc<[u8]>) {
        self.bake_files.baked(SUFFIX);
        self.navmesh = SavedNavmesh::new(Some(bytes));
    }
}

#[cfg(test)]
#[path = "saved_tests.rs"]
mod tests;
