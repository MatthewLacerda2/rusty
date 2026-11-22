use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Action, BakeFiles};

fn file(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = rusty_test_support::temp::dir().join("bake_files_refused");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn a_refused_file_is_removed_while_it_is_still_that_file() {
    let path = file("refused.occlusion", b"trunc");
    let files = BakeFiles::default();
    files.refused(&path, b"trunc", "test");
    let saved = files.store(&path, None, "T").unwrap();
    assert_eq!((saved.action, saved.warning), (Action::Removed, None));
    assert!(!path.exists());
}

#[test]
fn a_valid_file_baked_over_a_refused_one_is_kept_without_a_false_overwrite() {
    let path = file("refused_then_baked.occlusion", b"trunc");
    let files = BakeFiles::default();
    files.refused(&path, b"trunc", "test");
    std::fs::write(&path, b"a valid bake").unwrap();
    let saved = files.store(&path, None, "T").unwrap();
    assert_eq!(saved.action, Action::Kept);
    let warning = saved.warning.expect("says it keeps the newer bake");
    assert!(warning.contains("keeps that newer bake"), "{warning}");
    assert!(!warning.contains("overwrote"), "{warning}");
    assert_eq!(std::fs::read(&path).unwrap(), b"a valid bake");
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
    let other: Arc<[u8]> = Arc::from(&b"other"[..]);
    let saved = files.store_shared(&path, Some(&other), "T").unwrap();
    assert_eq!(saved.action, Action::Written);
    assert_eq!(std::fs::read(&path).unwrap(), b"other");
}

fn pathdiff(path: &Path, base: &Path) -> PathBuf {
    let (p, b): (Vec<_>, Vec<_>) = (path.components().collect(), base.components().collect());
    let shared = p.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let ups = (shared..b.len()).map(|_| std::path::Component::ParentDir);
    ups.chain(p[shared..].iter().copied()).collect()
}

#[test]
fn a_file_not_used_says_why_until_the_path_is_read_again() {
    let (path, fine) = (
        file("listed.occlusion", b"trunc"),
        file("fine.navmesh", b"ok"),
    );
    let files = BakeFiles::default();
    files.loaded(&fine, Some(b"ok"), Some(b"ok"));
    files.refused(&path, b"trunc", "occluder flag 30");
    assert_eq!(files.not_used(&fine), None, "a file read and kept was used");
    let why = files.not_used(&path);
    assert_eq!(why.as_deref(), Some("refused (occluder flag 30)"));
    files.loaded(&path, Some(b"good"), Some(b"good"));
    assert_eq!(files.not_used(&path), None, "read afresh: used");
    files.unreadable(&path, "permission denied");
    let why = files.not_used(&path);
    assert_eq!(
        why.as_deref(),
        Some("could not be read (permission denied)")
    );
}

#[test]
fn a_bake_or_a_copy_carries_no_reason_a_file_was_not_used() {
    let path = file("baked_over.occlusion", b"trunc");
    let files = BakeFiles::default();
    files.refused(&path, b"trunc", "truncated");
    let to = file("copied.occlusion", b"trunc");
    files.copied(&path, &to);
    assert_eq!(files.not_used(&to), None, "the copy was never read");
    files.baked(".occlusion");
    assert_eq!(files.not_used(&path), None, "this world's bake replaces it");
}
