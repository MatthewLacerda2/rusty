//! src/scene/nav_settings/saved.rs — the `<scene>.navmesh` file (#1887): the saved
//! navmesh beside the scene, Unity's baked `NavMeshData`.
//!
//! The scene only carries the file's bytes ([`Scene::navmesh`]): what they mean, and
//! whether they still match the scene, is `navigation`'s (the arrow is navigation →
//! scene). A save writes the bytes the scene holds to its own path, so *Save As*
//! carries the navmesh to the new path as it carries the baked PVS; a scene that holds
//! none leaves any file at its path alone (the file is checked on use, so a stale one
//! costs only a full bake). A save never bakes: `Navigation.Bake()` and the editor's
//! *Bake Navigation* button do.

use std::path::PathBuf;
use std::sync::Arc;

use crate::scene::Scene;

const SUFFIX: &str = ".navmesh";

/// Where `scene_path`'s navmesh is kept: `foo.scene` → `foo.scene.navmesh`.
pub fn navmesh_path(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{SUFFIX}"))
}

/// Write the navmesh `scene` holds beside `scene_path`; nothing when it holds none.
pub fn save_navmesh(scene: &Scene, scene_path: &str) -> Result<(), String> {
    let Some(bytes) = scene.navmesh.as_deref() else {
        return Ok(());
    };
    let path = navmesh_path(scene_path);
    // lint: allow(asset-write) the saved navmesh, a bake document read whole on load
    std::fs::write(&path, bytes).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Read the navmesh beside `scene_path` onto `scene`: none when there is no file,
/// and none, with a warning, when it cannot be read. Its content is checked when Play
/// uses it (`navigation`), never here.
pub fn load_navmesh(scene: &mut Scene, scene_path: &str) {
    let path = navmesh_path(scene_path);
    scene.navmesh = match std::fs::read(&path) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_navmesh_loads_back_and_none_leaves_the_file() {
        let dir = crate::test_temp::dir().join("rusty_navmesh_sidecar");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("n.scene").to_string_lossy().into_owned();
        let scene = Scene {
            navmesh: Some(Arc::from(vec![1u8, 2, 3])),
            ..Scene::default()
        };
        save_navmesh(&scene, &path).unwrap();
        save_navmesh(&Scene::default(), &path).unwrap();
        let mut back = Scene::default();
        load_navmesh(&mut back, &path);
        assert_eq!(back.navmesh.as_deref(), Some(&[1u8, 2, 3][..]));
        std::fs::remove_file(navmesh_path(&path)).unwrap();
        load_navmesh(&mut back, &path);
        assert!(back.navmesh.is_none(), "no file, no navmesh");
        std::fs::remove_dir_all(&dir).ok();
    }
}
