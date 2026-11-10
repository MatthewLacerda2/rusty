//! A `<scene>.occlusion` file that is there but cannot be read (#1714): it loads
//! no PVS, with a warning, and the next save keeps it rather than removing it as
//! if absent; a refused (corrupt) one is removed by the next save, so the error
//! clears itself (#1982). Only a missing file means none.

use super::{load_sidecar, occlusion_path, save_sidecar};
use crate::scene::io::Action;
use crate::scene::Scene;

fn level(name: &str) -> String {
    let dir = crate::test_temp::dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("level.scene").to_string_lossy().into_owned()
}

#[test]
fn an_unreadable_file_loads_nothing_and_a_save_keeps_it() {
    let scene_path = level("occlusion_unread");
    let path = occlusion_path(&scene_path);
    // A directory where the file goes: reading it fails, and not as "not found".
    std::fs::create_dir_all(&path).unwrap();
    let mut scene = Scene::new();
    load_sidecar(&mut scene, &scene_path);
    assert!(scene.occlusion.is_none());
    let saved = save_sidecar(&scene, &scene_path).expect("a save leaves it");
    assert_eq!((saved.action, saved.warning), (Action::Kept, None));
    assert!(path.exists(), "the unread file is kept");
    std::fs::remove_dir(&path).unwrap();
}

#[test]
fn a_refused_file_is_removed_by_the_next_save() {
    let scene_path = level("occlusion_refused");
    let path = occlusion_path(&scene_path);
    std::fs::write(&path, b"RPVS\x01").unwrap();
    let mut scene = Scene::new();
    load_sidecar(&mut scene, &scene_path);
    assert!(scene.occlusion.is_none(), "refused");
    let saved = save_sidecar(&scene, &scene_path).unwrap();
    assert_eq!(saved.action, Action::Removed);
    assert!(!path.exists(), "the corrupt file is gone");
}
