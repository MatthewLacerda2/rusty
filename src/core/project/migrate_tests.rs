//! The legacy migration's guards (#829, #890): the rewrite count it reports, that an
//! existing `saved/storage.json` is never overwritten, and that dot folders, `cache/`
//! and `saved/` are left alone.

use std::path::{Path, PathBuf};

use super::create_skeleton;
use super::migrate::legacy_layout;

/// A fresh legacy project root: build settings naming a `project/` path.
fn legacy(name: &str) -> PathBuf {
    let root = crate::test_temp::dir().join("project_migrate").join(name);
    std::fs::remove_dir_all(&root).ok();
    write(
        &root,
        "build_settings.json",
        r#"{"s": "project/scenes/a.scene"}"#,
    );
    root
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn the_report_counts_every_rewritten_file() {
    let root = legacy("count");
    write(&root, "scenes/a.scene", r#"{"p": "project/assets/a.lua"}"#);
    write(&root, "assets/b.lua", "return 'project/prefabs/B.prefab'");
    write(&root, "assets/plain.lua", "return {}");

    let lines = legacy_layout(&root).unwrap();
    // build_settings.json, a.scene and b.lua name a legacy path; plain.lua does not.
    assert_eq!(
        lines.last().unwrap(),
        "rewrote the stored paths in 3 file(s)",
        "{lines:?}"
    );
}

#[test]
fn a_newer_saved_storage_is_never_overwritten() {
    let root = legacy("storage");
    create_skeleton(&root).unwrap();
    write(&root, "storage.json", r#"{"old": 1}"#);
    write(&root, "saved/storage.json", r#"{"new": 2}"#);

    legacy_layout(&root).unwrap();
    assert_eq!(read(&root, "saved/storage.json"), r#"{"new": 2}"#);
    assert_eq!(
        read(&root, "storage.json"),
        r#"{"old": 1}"#,
        "left in place"
    );
}

#[test]
fn dot_cache_and_saved_folders_are_not_rewritten() {
    let root = legacy("skipped");
    let text = r#"{"p": "project/scenes/x.scene"}"#;
    let skipped = [".git/x.scene", "cache/x.scene", "saved/x.scene"];
    for rel in skipped {
        write(&root, rel, text);
    }

    legacy_layout(&root).unwrap();
    for rel in skipped {
        assert_eq!(read(&root, rel), text, "{rel} is byte-identical");
    }
    assert_eq!(
        read(&root, "build_settings.json"),
        r#"{"s": "assets/scenes/a.scene"}"#
    );
}
