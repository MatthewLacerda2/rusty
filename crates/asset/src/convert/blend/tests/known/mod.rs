//! The Blender version kept across processes (`known`), with a fake executable
//! that counts its runs: a fresh process (a new `Memo`) reads the kept answer and
//! starts no Blender.

use super::super::known::{self, Known, FILE, READS};
use super::super::locate::Memo;
use super::super::Search;
use super::locate::{script, search};
use std::path::{Path, PathBuf};

mod keys;

/// A fake Blender in `dir` that says `version` and counts each run in `dir/runs`.
pub(super) fn counting(dir: &Path, version: &str) -> PathBuf {
    let runs = dir.join("runs");
    script(
        dir,
        &format!("echo run >> '{}'\necho 'Blender {version}'", runs.display()),
    )
}

pub(super) fn runs(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join("runs")).map_or(0, |s| s.lines().count())
}

/// A search finding the fake at `blender`, keeping its answer in `dir/cache/`.
pub(super) fn kept_in(dir: &Path, blender: &Path) -> (Search, PathBuf) {
    let file = dir.join("cache").join(FILE);
    let search = Search {
        known: Some(file.clone()),
        ..search(Some(blender), None)
    };
    (search, file)
}

/// The version `search` finds in a fresh process.
pub(super) fn fresh(search: &Search) -> Result<String, String> {
    let found = search.find_in(&Memo::default());
    found.map(|b| b.version).map_err(|e| e.to_string())
}

#[test]
fn a_second_process_reads_the_kept_version_and_starts_no_blender() {
    let dir = rusty_test_support::temp::dir();
    let blender = counting(&dir.join("bin"), "5.0.1");
    let (search, file) = kept_in(&dir, &blender);
    assert_eq!(fresh(&search).as_deref(), Ok("5.0.1"));
    let entry = known::read(&file).expect("the first ask is kept");
    assert_eq!(entry.path, blender);
    assert_eq!(entry.canonical, std::fs::canonicalize(&blender).unwrap());
    assert_eq!(fresh(&search).as_deref(), Ok("5.0.1"));
    let bin = dir.join("bin");
    assert_eq!(runs(&bin), 1, "the second process read the file");
}

#[test]
fn an_answer_read_its_last_time_is_asked_again() {
    let dir = rusty_test_support::temp::dir();
    let blender = counting(&dir.join("bin"), "5.0.1");
    let (search, file) = kept_in(&dir, &blender);
    fresh(&search).unwrap();
    let worn = Known {
        reads: READS - 1,
        ..known::read(&file).unwrap()
    };
    known::write(&file, &worn).unwrap();
    fresh(&search).unwrap();
    assert_eq!(runs(&dir.join("bin")), 1, "one read was left");
    assert_eq!(known::read(&file).unwrap().reads, READS);
    fresh(&search).unwrap();
    assert_eq!(runs(&dir.join("bin")), 2, "none was left");
    assert_eq!(known::read(&file).unwrap().reads, 0, "asked anew");
}

#[test]
fn a_garbage_file_is_a_miss_and_is_rewritten() {
    let dir = rusty_test_support::temp::dir();
    let blender = counting(&dir.join("bin"), "5.0.1");
    let (search, file) = kept_in(&dir, &blender);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{\"path\": \"/half-writ").unwrap();
    assert_eq!(fresh(&search).as_deref(), Ok("5.0.1"));
    assert_eq!(known::read(&file).unwrap().version, "5.0.1");
}

#[test]
fn a_refused_version_is_never_kept() {
    let dir = rusty_test_support::temp::dir();
    let blender = counting(&dir.join("bin"), "4.2.3");
    let (search, file) = kept_in(&dir, &blender);
    let first = fresh(&search).expect_err("4 is refused");
    assert!(first.contains("is Blender 4.2.3"), "{first}");
    assert!(!file.exists(), "a refusal is asked again by each process");
    assert_eq!(fresh(&search), Err(first));
    assert_eq!(runs(&dir.join("bin")), 2);
}

#[test]
fn no_answer_and_no_version_are_never_kept() {
    let dir = rusty_test_support::temp::dir();
    let silent = script(&dir.join("silent"), "echo hello");
    let (search, file) = kept_in(&dir, &silent);
    assert!(fresh(&search).is_err());
    assert!(!file.exists(), "an output naming no version");
    let unstartable = dir.join("broken").join("blender");
    std::fs::create_dir_all(unstartable.parent().unwrap()).unwrap();
    std::fs::write(&unstartable, "not a program").unwrap();
    let (search, file) = kept_in(&dir, &unstartable);
    assert!(fresh(&search).is_err());
    assert!(!file.exists(), "a Blender that could not start");
}

#[test]
fn only_a_project_keeps_the_version() {
    let dir = rusty_test_support::temp::dir();
    assert_eq!(known::in_project(&dir), None);
    std::fs::write(dir.join(rusty_core::project::PROJECT_FILE), "").unwrap();
    let file = dir.join(rusty_core::project::CACHE_DIR).join(FILE);
    assert_eq!(known::in_project(&dir), Some(file));
}
