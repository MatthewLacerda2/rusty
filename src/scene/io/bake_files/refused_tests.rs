//! A refused file and a path's spellings (#1982's second review): a file this world
//! refused is rewritten or removed only while it is still that file, and a relative
//! and an absolute spelling of one path find one record.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Action, BakeFiles};

fn file(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = crate::test_temp::dir().join("bake_files_refused");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn a_refused_file_is_removed_while_it_is_still_that_file() {
    let path = file("refused.occlusion", b"trunc");
    let files = BakeFiles::default();
    files.refused(&path, b"trunc");
    let saved = files.store(&path, None, "T").unwrap();
    assert_eq!((saved.action, saved.warning), (Action::Removed, None));
    assert!(!path.exists());
}

#[test]
fn a_valid_file_baked_over_a_refused_one_is_kept_without_a_false_overwrite() {
    let path = file("refused_then_baked.occlusion", b"trunc");
    let files = BakeFiles::default();
    files.refused(&path, b"trunc");
    // Another process bakes a valid PVS there.
    std::fs::write(&path, b"a valid bake").unwrap();
    let saved = files.store(&path, None, "T").unwrap();
    assert_eq!(saved.action, Action::Kept);
    let warning = saved.warning.expect("says it keeps the newer bake");
    assert!(warning.contains("keeps that newer bake"), "{warning}");
    assert!(!warning.contains("overwrote"), "{warning}");
    assert_eq!(std::fs::read(&path).unwrap(), b"a valid bake");
    // This world baking since does write over it.
    files.baked(".occlusion");
    let saved = files.store(&path, Some(b"mine"), "T").unwrap();
    assert_eq!(saved.action, Action::Written);
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
}

#[test]
fn a_relative_and_an_absolute_spelling_share_one_record() {
    let path = file("spelled.navmesh", b"old");
    let cwd = std::env::current_dir().unwrap();
    let relative = pathdiff(&path, &cwd);
    let files = BakeFiles::default();
    files.loaded(&relative, Some(b"old"), Some(b"old"));
    assert!(
        files.has(&path),
        "{} and {}",
        relative.display(),
        path.display()
    );
    std::fs::write(&path, b"new").unwrap();
    let saved = files.store(&path, Some(b"old"), "T").unwrap();
    assert_eq!(saved.action, Action::Kept, "the newer file is kept");
    assert!(files.warned(&relative));
}

#[test]
fn a_held_shared_buffer_is_kept_as_held() {
    let path = file("shared.navmesh", b"bytes");
    let held: Arc<[u8]> = Arc::from(&b"bytes"[..]);
    let files = BakeFiles::default();
    files.loaded_shared(&path, Some(&held));
    let saved = files.store_shared(&path, Some(&held), "T").unwrap();
    assert_eq!((saved.action, saved.warning), (Action::Kept, None));
    // Other bytes in a new buffer are hashed, and written.
    let other: Arc<[u8]> = Arc::from(&b"other"[..]);
    let saved = files.store_shared(&path, Some(&other), "T").unwrap();
    assert_eq!(saved.action, Action::Written);
    assert_eq!(std::fs::read(&path).unwrap(), b"other");
}

/// `path` relative to `base`, through `..` where it must: the spelling a scene
/// opened as `scenes/level.scene` gives its sidecars.
fn pathdiff(path: &Path, base: &Path) -> PathBuf {
    let (p, b): (Vec<_>, Vec<_>) = (path.components().collect(), base.components().collect());
    let shared = p.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let ups = (shared..b.len()).map(|_| std::path::Component::ParentDir);
    ups.chain(p[shared..].iter().copied()).collect()
}
