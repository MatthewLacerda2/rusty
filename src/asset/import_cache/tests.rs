//! The import cache (#857): one parse per file, as fresh as a re-import.

use super::*;
use crate::asset::fixtures::TRIANGLE_OBJ;
use crate::asset::{import_file, sidecar};
use std::path::PathBuf;

pub(super) fn model(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join(format!("rusty_import_cache_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("quad.obj");
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    path
}

#[test]
fn a_file_is_parsed_once_and_shared() {
    let path = model("once");
    let before = parses();
    let first = imported(&path).unwrap();
    for _ in 0..40 {
        let again = imported(&path).unwrap();
        assert!(Arc::ptr_eq(&first.asset, &again.asset));
        assert_eq!(
            again.revision, first.revision,
            "instances share one revision"
        );
    }
    assert_eq!(parses() - before, 1);
}

#[test]
fn a_cached_import_is_the_fresh_one() {
    let path = model("same");
    let cached = imported(&path).unwrap();
    let fresh = import_file(&path).unwrap();
    assert_eq!(*cached.asset, fresh);
}

#[test]
fn a_written_source_is_parsed_again_and_its_sidecar_is_not_followed() {
    let path = model("written");
    let first = imported(&path).unwrap();
    crate::core::fs::written::note_written(&path.to_string_lossy());
    let second = imported(&path).unwrap();
    assert!(
        second.revision > first.revision,
        "a real re-import is new geometry"
    );
    // A sidecar save that keeps the geometry settings keeps the import (#857).
    let meta = sidecar::meta_path(&path);
    crate::core::fs::written::note_written(&meta.to_string_lossy());
    assert_eq!(imported(&path).unwrap().revision, second.revision);
    let before = parses();
    sidecar::import_and_sync_sidecar(&path).unwrap();
    let modified = std::fs::metadata(&meta).unwrap().modified().unwrap();
    sidecar::import_and_sync_sidecar(&path).unwrap();
    let again = std::fs::metadata(&meta).unwrap().modified().unwrap();
    assert_eq!(modified, again, "an unchanged sidecar is not re-written");
    assert_eq!(parses() - before, 0);
}

#[test]
fn an_edited_mtl_is_parsed_again() {
    let path = model("mtl");
    let mtl = path.with_file_name("quad.mtl");
    std::fs::write(&mtl, "newmtl Red\nKd 1 0 0\n").unwrap();
    let obj = format!("mtllib quad.mtl\n{TRIANGLE_OBJ}");
    std::fs::write(&path, obj).unwrap();
    let red = imported(&path).unwrap();
    assert_eq!(red.materials[0].base_color[1], 0.0);
    // Noted by the engine: the write log alone evicts it.
    std::fs::write(&mtl, "newmtl Red\nKd 1 1 0\n").unwrap();
    crate::core::fs::written::note_written(&mtl.to_string_lossy());
    let yellow = imported(&path).unwrap();
    assert_eq!(yellow.materials[0].base_color[1], 1.0);
    // Re-written by another process to another length: its stamp does.
    std::fs::write(&mtl, "newmtl Red\nKd 1 0.5 0\n").unwrap();
    assert_eq!(imported(&path).unwrap().materials[0].base_color[1], 0.5);
}

#[test]
fn a_geometry_setting_is_a_different_key() {
    let path = model("settings");
    let unit = imported(&path).unwrap();
    sidecar::set_scale(&path, 2.0).unwrap();
    let doubled = imported(&path).unwrap();
    assert!(doubled.revision > unit.revision);
    let x = |i: &Imported| i.sub_meshes[0].vertices[0].position[0];
    assert_eq!(x(&doubled), 2.0 * x(&unit));
}

#[test]
fn a_file_another_process_rewrote_is_parsed_again() {
    let path = model("external");
    let first = imported(&path).unwrap();
    // Not noted in the write log: only its size and time say it changed.
    let text = TRIANGLE_OBJ.replace("o Quad", "o Renamed");
    std::fs::write(&path, text).unwrap();
    let second = imported(&path).unwrap();
    assert_eq!(second.sub_mesh_ids(), vec!["Renamed".to_string()]);
    assert!(second.revision > first.revision);
}

#[test]
fn a_failed_import_is_not_kept() {
    let dir = crate::test_temp::dir().join("rusty_import_cache_missing");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("late.obj");
    let _ = std::fs::remove_file(&path);
    assert!(imported(&path).is_err());
    std::fs::write(&path, TRIANGLE_OBJ).unwrap();
    assert!(imported(&path).is_ok(), "the file that appeared is read");
    std::fs::remove_file(&path).unwrap();
    assert!(imported(&path).is_err(), "a vanished file is not served");
}

#[test]
fn another_project_drops_every_entry() {
    let path = model("project");
    let first = imported(&path).unwrap();
    CACHE.with_borrow_mut(|c| c.enter(u64::MAX));
    assert!(CACHE.with_borrow(|c| c.by_path.is_empty()));
    assert!(imported(&path).unwrap().revision > first.revision);
}

#[test]
fn a_converted_source_hits_with_its_source_gone() {
    // As an exported game ships it (`dev::export`): the cached `.glb`, no `.fbx`.
    let path = crate::test_temp::dir().join("shipped.fbx");
    let fbx = include_str!("../../../tests/fixtures/models/embedded_textures.fbx");
    std::fs::write(&path, fbx).unwrap();
    let first = imported(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let before = parses();
    let again = imported(&path).unwrap();
    assert_eq!(again.revision, first.revision);
    assert_eq!(parses(), before, "served from the cache");
}
