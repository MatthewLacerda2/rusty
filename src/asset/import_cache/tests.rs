//! The import cache (#857): one parse per file, as fresh as a re-import.

use super::*;
use crate::asset::fixtures::TRIANGLE_OBJ;
use crate::asset::{import_file, sidecar};
use std::path::PathBuf;

fn model(name: &str) -> PathBuf {
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
fn a_written_source_or_sidecar_is_parsed_again() {
    let path = model("written");
    let first = imported(&path).unwrap();
    crate::core::fs::written::note_written(&path.to_string_lossy());
    let second = imported(&path).unwrap();
    assert!(!Arc::ptr_eq(&first.asset, &second.asset));
    assert!(
        second.revision > first.revision,
        "a real re-import is new geometry"
    );
    let meta = sidecar::meta_path(&path);
    crate::core::fs::written::note_written(&meta.to_string_lossy());
    assert!(imported(&path).unwrap().revision > second.revision);
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
fn past_the_budget_the_least_recently_used_goes() {
    let asset = import_file(&model("budget")).unwrap();
    let size = approx_bytes(&asset);
    assert!(size > 0);
    let geometry = Geometry::of(&ImportSettings::default());
    let mut cache = Cache::default();
    let put = |cache: &mut Cache, key: &str| {
        let stamp = Some((1, None));
        cache.insert(key.into(), geometry, stamp, asset.clone(), 2 * size);
    };
    put(&mut cache, "a");
    put(&mut cache, "b");
    assert!(
        cache.hit("a", geometry, Some((1, None))).is_some(),
        "a is used last"
    );
    put(&mut cache, "c");
    let held: Vec<_> = cache.by_path.keys().cloned().collect();
    assert_eq!(held, ["a", "c"], "b, least recently used, went");
    assert_eq!(cache.bytes, 2 * size);
    // The newest entry stays even when it alone is over the budget.
    cache.insert("big".into(), geometry, None, asset.clone(), size / 2);
    assert_eq!(cache.by_path.len(), 1);
}

#[test]
fn another_project_drops_every_entry() {
    let path = model("project");
    let first = imported(&path).unwrap();
    CACHE.with_borrow_mut(|c| c.enter(u64::MAX));
    assert!(CACHE.with_borrow(|c| c.by_path.is_empty()));
    assert!(imported(&path).unwrap().revision > first.revision);
}
