//! The `<scene>.navmesh` sidecar (#1887): it loads back, a scene holding none leaves
//! the file alone, and a save that would write the bytes the file holds does not.

use std::sync::Arc;

use super::*;

fn holding(bytes: &[u8]) -> Scene {
    Scene {
        navmesh: SavedNavmesh::new(Some(Arc::from(bytes))),
        ..Scene::default()
    }
}

#[test]
fn a_saved_navmesh_loads_back_and_none_leaves_the_file() {
    let dir = crate::test_temp::dir().join("rusty_navmesh_sidecar");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("n.scene").to_string_lossy().into_owned();
    assert!(save_navmesh(&holding(&[1, 2, 3]), &path).unwrap());
    assert!(!save_navmesh(&Scene::default(), &path).unwrap());
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
    assert!(
        save_navmesh(&scene, &path).unwrap(),
        "the first save writes"
    );
    assert!(
        !save_navmesh(&scene, &path).unwrap(),
        "the same bytes: no write"
    );
    let mut loaded = Scene::default();
    load_navmesh(&mut loaded, &path);
    assert!(
        !save_navmesh(&loaded, &path).unwrap(),
        "as loaded: no write"
    );
    let other = dir.join("m.scene").to_string_lossy().into_owned();
    assert!(
        save_navmesh(&loaded, &other).unwrap(),
        "Save As writes the new path"
    );
    std::fs::remove_file(navmesh_path(&path)).unwrap();
    assert!(
        save_navmesh(&loaded, &path).unwrap(),
        "a deleted file is written again"
    );
    loaded.navmesh.bytes = Some(Arc::from(vec![9u8, 9, 9]));
    assert!(
        save_navmesh(&loaded, &path).unwrap(),
        "new bytes are written"
    );
    let mut back = Scene::default();
    load_navmesh(&mut back, &path);
    assert_eq!(back.navmesh.bytes.as_deref(), Some(&[9u8, 9, 9][..]));
    std::fs::remove_dir_all(&dir).ok();
}
