use super::{load_sidecar, occlusion_path, save_sidecar};
use crate::io::Action;
use crate::Scene;

fn level(name: &str) -> String {
    let dir = rusty_test_support::temp::dir().join(name);
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

/// The layout before the version field: `RPVS`, `objects` zeroed objects, an empty grid,
/// no row words and no cells.
fn unversioned(objects: u32) -> Vec<u8> {
    let mut bytes = b"RPVS".to_vec();
    bytes.extend(objects.to_le_bytes());
    bytes.extend(std::iter::repeat_n(0u8, objects as usize * 13 + 28));
    bytes.extend([0u8; 8]);
    bytes
}

#[test]
fn a_file_from_before_the_version_field_is_told_as_older_and_unversioned() {
    let scene_path = level("occlusion_unversioned");
    // A count above the version would read "newer", one equal to it would fail a later field.
    for objects in [137, super::OCCLUSION_VERSION] {
        std::fs::write(occlusion_path(&scene_path), unversioned(objects)).unwrap();
        let mut scene = Scene::new();
        let logs = rusty_test_support::logs::during(|| load_sidecar(&mut scene, &scene_path));
        assert!(scene.occlusion.is_none(), "refused");
        let logs = logs.join("\n");
        assert!(
            logs.contains("is an occlusion bake from an older engine, which wrote no version"),
            "{objects} objects: {logs}"
        );
        assert!(!logs.contains("newer"), "{logs}");
        assert!(
            logs.contains("re-baked (Generate Lighting or Bake Occlusion)"),
            "{logs}"
        );
    }
}
