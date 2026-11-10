//! The bake files' rule (#1982) on raw bytes: a save writes only what this world
//! baked, never writes back what it only loaded over a newer file, restores a file
//! removed outside, and removes a newer file never.

use std::path::{Path, PathBuf};

use super::{Action, BakeFiles};

fn file(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = crate::test_temp::dir().join("bake_files");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

fn loaded(path: &Path) -> BakeFiles {
    let files = BakeFiles::default();
    let bytes = std::fs::read(path).unwrap();
    files.loaded(path, Some(&bytes), Some(&bytes));
    files
}

#[test]
fn as_loaded_is_not_written_and_a_newer_file_is_kept_with_one_warning() {
    let path = file("newer.occlusion", b"old");
    let files = loaded(&path);
    let saved = files.store(&path, Some(b"old"), "T").unwrap();
    assert_eq!((saved.action, saved.warning), (Action::Kept, None));
    // Another process bakes: the same length, so only the hash tells.
    std::fs::write(&path, b"new").unwrap();
    let saved = files.store(&path, Some(b"old"), "T").unwrap();
    assert_eq!(saved.action, Action::Kept);
    assert!(saved.warning.is_some_and(|w| w.contains("newer bake")));
    let again = files.store(&path, Some(b"old"), "T").unwrap();
    assert_eq!((again.action, again.warning), (Action::Kept, None), "once");
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

#[test]
fn a_bake_since_is_written_over_a_newer_file_and_says_so() {
    let path = file("baked.occlusion", b"old");
    let files = loaded(&path);
    std::fs::write(&path, b"theirs").unwrap();
    let saved = files.store(&path, Some(b"mine"), "T").unwrap();
    assert_eq!(saved.action, Action::Written);
    assert!(saved.warning.is_some_and(|w| w.contains("overwrote")));
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    let again = files.store(&path, Some(b"mine"), "T").unwrap();
    assert_eq!(again.action, Action::Kept, "written: now as last written");
}

#[test]
fn a_rebake_of_the_same_bytes_is_still_written() {
    let path = file("rebaked.occlusion", b"old");
    let files = loaded(&path);
    std::fs::write(&path, b"theirs").unwrap();
    files.baked(".occlusion");
    let saved = files.store(&path, Some(b"old"), "T").unwrap();
    assert_eq!(saved.action, Action::Written);
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
}

#[test]
fn save_as_writes_the_new_path() {
    let path = file("from.occlusion", b"old");
    let other = file("to.occlusion", b"stale");
    let files = loaded(&path);
    let saved = files.store(&other, Some(b"old"), "T").unwrap();
    assert_eq!((saved.action, saved.warning), (Action::Written, None));
    assert_eq!(std::fs::read(&other).unwrap(), b"old");
}

#[test]
fn a_cleared_bake_removes_only_the_file_this_world_saw_unchanged() {
    let path = file("cleared.occlusion", b"old");
    let files = loaded(&path);
    assert_eq!(
        files.store(&path, None, "T").unwrap().action,
        Action::Removed
    );
    assert!(!path.exists());

    let unseen = file("unseen.occlusion", b"theirs");
    let saved = BakeFiles::default().store(&unseen, None, "T").unwrap();
    assert_eq!((saved.action, saved.warning), (Action::Removed, None));
    assert!(
        !unseen.exists(),
        "no record: a path saved over gets what is held"
    );

    let changed = file("changed.occlusion", b"old");
    let files = loaded(&changed);
    std::fs::write(&changed, b"newer").unwrap();
    let saved = files.store(&changed, None, "T").unwrap();
    assert_eq!(saved.action, Action::Kept);
    assert!(saved.warning.is_some());
    assert_eq!(std::fs::read(&changed).unwrap(), b"newer");
}

#[test]
fn a_file_written_where_none_was_read_is_newer_too() {
    let dir = crate::test_temp::dir().join("bake_files");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("appeared.navmesh");
    std::fs::remove_file(&path).ok();
    let files = BakeFiles::default();
    files.loaded(&path, None, None);
    std::fs::write(&path, b"theirs").unwrap();
    let saved = files.store(&path, None, "T").unwrap();
    assert_eq!(saved.action, Action::Kept);
    assert!(saved.warning.is_some());
    assert!(path.exists());
}

#[test]
fn a_file_removed_outside_is_written_back_when_a_bake_is_held() {
    let path = file("removed.navmesh", b"mine");
    let files = loaded(&path);
    std::fs::remove_file(&path).unwrap();
    let saved = files.store(&path, Some(b"mine"), "T").unwrap();
    assert_eq!(saved.action, Action::Written);
    assert!(saved.warning.is_some_and(|w| w.contains("removed outside")));
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    std::fs::remove_file(&path).unwrap();
    let saved = files.store(&path, None, "T").unwrap();
    assert_eq!((saved.action, saved.warning), (Action::Kept, None));
}
