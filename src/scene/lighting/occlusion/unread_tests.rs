//! A `<scene>.occlusion` file that is there but cannot be read (#1714): it loads
//! no PVS, with a warning, and the next save keeps it rather than removing it as
//! if absent. Only a missing file means none.

use super::{load_sidecar, occlusion_path, save_sidecar};
use crate::scene::Scene;

#[test]
fn an_unreadable_file_loads_nothing_and_a_save_keeps_it() {
    let dir = crate::test_temp::dir().join("occlusion_unread");
    std::fs::create_dir_all(&dir).unwrap();
    let scene_path = dir.join("level.scene").to_string_lossy().into_owned();
    let path = occlusion_path(&scene_path);
    // A directory where the file goes: reading it fails, and not as "not found".
    std::fs::create_dir_all(&path).unwrap();
    let mut scene = Scene::new();
    load_sidecar(&mut scene, &scene_path);
    assert!(scene.occlusion.is_none() && scene.occlusion_unread);
    save_sidecar(&scene, &scene_path).expect("a save leaves it");
    assert!(path.exists(), "the unread file is kept");
    std::fs::remove_dir(&path).unwrap();
    load_sidecar(&mut scene, &scene_path);
    assert!(!scene.occlusion_unread, "a missing file is just absent");
}
