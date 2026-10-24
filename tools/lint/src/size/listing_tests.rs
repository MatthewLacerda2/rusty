//! The full scan measures what can be committed (#1373), and walks the disk where
//! git can't say.

use super::super::{check_file, fallback_note, full_scan, MAX_TEST_FILE_LINES};
use super::committable;
use crate::repo::scaffold::Repo;
use std::fs;

/// One line over the fixture cap, in Lua.
fn too_long() -> String {
    "x = 1\n".repeat(MAX_TEST_FILE_LINES + 1)
}

#[test]
fn a_seeded_ignored_fixture_passes_and_an_untracked_new_file_is_flagged() {
    let repo = Repo::new("1373-git", true);
    repo.file(".gitignore", "/tests/fixtures/project/assets/*\ntarget/\n");
    repo.file(
        "tests/fixtures/project/assets/scripts/ui/kit.lua",
        &too_long(),
    );
    repo.file("tools/lint/target/gen.rs", &"//\n".repeat(400));
    let tracked = repo.file("src/a.rs", "//\n");
    repo.git(&["add", "src/a.rs"]);
    let new = repo.file("tests/new.lua", &too_long());
    let found = full_scan(&repo.0);
    assert_eq!(found, vec![tracked, new.clone()]);
    let flagged: Vec<_> = found.iter().filter_map(|p| check_file(p)).collect();
    assert_eq!(flagged.len(), 1, "{flagged:?}");
    assert!(flagged[0].contains("tests/new.lua"), "{}", flagged[0]);
}

#[test]
fn a_tracked_file_deleted_from_disk_is_not_listed() {
    let repo = Repo::new("1373-deleted", true);
    let kept = repo.file("src/a.rs", "//\n");
    let gone = repo.file("src/b.rs", "//\n");
    repo.git(&["add", "src"]);
    fs::remove_file(gone).expect("remove");
    assert_eq!(committable(&repo.0, &["src"]), Ok(vec![kept]));
}

#[test]
fn without_a_repository_the_scan_walks_the_disk() {
    // A tarball has no `.git`: git can't list, so every file on disk is measured,
    // build output still aside.
    let dir = Repo::new("1373-tarball", false);
    let why = committable(&dir.0, &["tests"]).expect_err("no repository");
    assert!(why.contains("not a git repository"), "{why}");
    let seeded = dir.file("tests/fixtures/project/assets/kit.lua", &too_long());
    dir.file("tests/target/gen.rs", "//\n");
    assert_eq!(full_scan(&dir.0), vec![seeded]);
}

#[test]
fn the_fallback_note_keeps_gits_message_on_its_own_lines() {
    let note = fallback_note("git exit status: 128: fatal: dubious\n  line two\n  line three");
    let mut lines = note.lines();
    let first = lines.next().expect("note");
    assert!(first.ends_with("gitignored ones included"), "{first}");
    assert!(!first.contains("dubious"), "{first}");
    assert_eq!(lines.count(), 4, "{note}");
}
