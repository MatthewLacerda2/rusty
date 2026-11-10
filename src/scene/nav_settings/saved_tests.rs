//! The `<scene>.navmesh` sidecar (#1887): it loads back, a scene holding none leaves
//! a file it never saw alone, and a save that would write the bytes it last read or
//! wrote there does not (#1982, `scene::io::BakeFiles`).

use std::sync::Arc;

use super::*;
use crate::scene::io::Action;

fn holding(bytes: &[u8]) -> Scene {
    Scene {
        navmesh: SavedNavmesh::new(Some(Arc::from(bytes))),
        ..Scene::default()
    }
}

fn action(scene: &Scene, path: &str) -> Action {
    save_navmesh(scene, path).unwrap().action
}

#[test]
fn a_saved_navmesh_loads_back_and_none_leaves_the_file() {
    let dir = crate::test_temp::dir().join("rusty_navmesh_sidecar");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("n.scene").to_string_lossy().into_owned();
    assert_eq!(action(&holding(&[1, 2, 3]), &path), Action::Written);
    assert_eq!(action(&Scene::default(), &path), Action::Kept);
    let mut back = Scene::default();
    load_navmesh(&mut back, &path);
    assert_eq!(back.navmesh.bytes.as_deref(), Some(&[1u8, 2, 3][..]));
    std::fs::remove_file(navmesh_path(&path)).unwrap();
    load_navmesh(&mut back, &path);
    assert!(back.navmesh.bytes.is_none(), "no file, no navmesh");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_unchanged_navmesh_is_not_written_again() {
    let dir = crate::test_temp::dir().join("rusty_navmesh_unchanged");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("n.scene").to_string_lossy().into_owned();
    let scene = holding(&[4, 5, 6]);
    assert_eq!(
        action(&scene, &path),
        Action::Written,
        "the first save writes"
    );
    assert_eq!(
        action(&scene, &path),
        Action::Kept,
        "the same bytes: no write"
    );
    let mut loaded = Scene::default();
    load_navmesh(&mut loaded, &path);
    assert_eq!(action(&loaded, &path), Action::Kept, "as loaded: no write");
    let other = dir.join("m.scene").to_string_lossy().into_owned();
    assert_eq!(action(&loaded, &other), Action::Written, "Save As writes");
    std::fs::remove_file(navmesh_path(&path)).unwrap();
    assert_eq!(
        action(&loaded, &path),
        Action::Kept,
        "a file deleted outside since the load stays deleted"
    );
    loaded.navmesh.bytes = Some(Arc::from(vec![9u8, 9, 9]));
    assert_eq!(
        action(&loaded, &path),
        Action::Written,
        "new bytes are written"
    );
    let mut back = Scene::default();
    load_navmesh(&mut back, &path);
    assert_eq!(back.navmesh.bytes.as_deref(), Some(&[9u8, 9, 9][..]));
    std::fs::remove_dir_all(&dir).ok();
}
