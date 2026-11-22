use super::super::{check_file, fallback_note, full_scan, MAX_TEST_FILE_LINES};
use super::committable;
use crate::repo::scaffold::Repo;
use std::fs;
use std::path::Path;

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
    let found = full_scan(&repo.0).0;
    assert_eq!(found, vec![tracked, new.clone()]);
    let flagged: Vec<_> = found
        .iter()
        .filter_map(|p| check_file(Path::new(""), p))
        .collect();
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
    let dir = Repo::new("1373-tarball", false);
    let why = committable(&dir.0, &["tests"]).expect_err("no repository");
    assert!(why.contains("not a git repository"), "{why}");
    let seeded = dir.file("tests/fixtures/project/assets/kit.lua", &too_long());
    dir.file("tests/target/gen.rs", "//\n");
    assert_eq!(full_scan(&dir.0).0, vec![seeded]);
}

#[test]
fn the_full_scan_reads_the_whole_tree_with_git_and_without() {
    let repo = Repo::new("2491-tree", true);
    repo.file(".gitignore", ".claude/worktrees/\n");
    let mut want = vec![
        repo.file(".github/scripts/m.py", "x = 1\n"),
        repo.file("build.rs", "fn main() {}\n"),
        repo.file("crates/a/README.md", "# A\n"),
        repo.file("games/smoke/main.lua", "x = 1\n"),
    ];
    repo.file("docs/a.md", "# Not read\n");
    repo.file(".claude/worktrees/b/src/a.rs", "//\n");
    assert_eq!(full_scan(&repo.0).0, want);
    let dir = Repo::new("2491-tree-disk", false);
    want = vec![dir.file("build.rs", "fn main() {}\n")];
    dir.file(".claude/worktrees/b/src/a.rs", "//\n");
    dir.file(".git/hooks/x.py", "x = 1\n");
    want.push(dir.file(".github/scripts/m.py", "x = 1\n"));
    want.push(dir.file("crates/a/README.md", "# A\n"));
    dir.file("docs/a.md", "# Not read\n");
    let mut found = full_scan(&dir.0).0;
    found.sort();
    want.sort();
    assert_eq!(
        found, want,
        "the disk walk skips the hidden folders git would not list"
    );
}

#[test]
fn the_disk_walk_skips_the_root_folders_the_gitignore_anchors() {
    let dir = Repo::new("2491-ignored-roots", false);
    dir.file(
        ".gitignore",
        "/project/\n/out/\n/tests/fixtures/*/out/\ntarget/\n",
    );
    dir.file("project/assets/a.lua", "x = 1\n");
    dir.file("out/gen.rs", "//\n");
    let mut want = vec![
        dir.file("crates/out/a.rs", "//\n"),
        dir.file("tests/fixtures/f/out/b.rs", "//\n"),
        dir.file("lib.rs", "//\n"),
    ];
    let mut found = full_scan(&dir.0).0;
    found.sort();
    want.sort();
    assert_eq!(
        found, want,
        "only a root folder the rules anchor is left out"
    );
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
