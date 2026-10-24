//! The full scan measures what can be committed (#1373), and walks the disk where
//! git can't say.

use super::super::{check_file, full_scan, MAX_TEST_FILE_LINES};
use super::committable;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch directory under the temp dir, removed when dropped; a git repository
/// when `git` is set.
struct Repo(PathBuf);

impl Repo {
    fn new(tag: &str, git: bool) -> Repo {
        let dir = std::env::temp_dir().join(format!("rusty-lint-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        if git {
            Repo::git(&dir, &["init", "-q"]);
        }
        Repo(dir)
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git");
        assert!(ok.success(), "git {args:?}");
    }

    fn file(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        fs::write(&path, body).expect("write");
        path
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}

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
    Repo::git(&repo.0, &["add", "src/a.rs"]);
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
    Repo::git(&repo.0, &["add", "src"]);
    fs::remove_file(gone).expect("remove");
    assert_eq!(committable(&repo.0, &["src"]), Some(vec![kept]));
}

#[test]
fn without_a_repository_the_scan_walks_the_disk() {
    // A tarball has no `.git`: git can't list, so every file on disk is measured,
    // build output still aside.
    let dir = Repo::new("1373-tarball", false);
    assert_eq!(committable(&dir.0, &["tests"]), None);
    let seeded = dir.file("tests/fixtures/project/assets/kit.lua", &too_long());
    dir.file("tests/target/gen.rs", "//\n");
    assert_eq!(full_scan(&dir.0), vec![seeded]);
}
