//! Finding Blender, with fake executables (#1072): what each refusal says.

use super::super::locate::parse_version;
use super::super::Search;
use std::path::{Path, PathBuf};

/// A fake `blender` in `dir` that prints `output` when asked its version.
pub(super) fn fake(dir: &Path, output: &str) -> PathBuf {
    script(dir, &format!("echo '{output}'"))
}

/// A fake `blender` in `dir` that runs the shell `body`. Written under another
/// name, closed, then renamed into place: a process forked meanwhile by another
/// test can't hold it open for writing ("text file busy").
pub(super) fn script(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).unwrap();
    let (draft, path) = (dir.join("blender.draft"), dir.join("blender"));
    std::fs::write(&draft, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&draft, std::fs::Permissions::from_mode(0o755)).unwrap();
    // lint: allow(atomic-write) the draft is executable before it takes the name, which `write_atomic` can't do
    std::fs::rename(&draft, &path).unwrap();
    path
}

pub(super) fn search(explicit: Option<&Path>, path: Option<&Path>) -> Search {
    Search {
        explicit: explicit.map(Into::into),
        path: path.map(Into::into),
        places: Vec::new(),
        timeout: None,
    }
}

fn error(search: &Search) -> String {
    search.find().expect_err("refused").to_string()
}

#[test]
fn no_blender_anywhere_says_so_and_where_to_get_it() {
    let empty = crate::test_temp::dir();
    let why = error(&search(None, Some(&empty)));
    assert!(why.contains("no Blender was found"), "{why}");
    assert!(why.contains("Blender 5 or later"), "{why}");
    assert!(why.contains("blender.org/download"), "{why}");
    assert!(why.contains("RUSTY_BLENDER"), "{why}");
}

#[test]
fn an_override_naming_no_file_is_refused_by_name() {
    let missing = crate::test_temp::dir().join("nowhere/blender");
    let why = error(&search(Some(&missing), None));
    assert!(why.contains(&missing.display().to_string()), "{why}");
    assert!(why.contains("not a file"), "{why}");
}

#[test]
fn a_blender_older_than_5_is_refused_with_its_version() {
    let dir = crate::test_temp::dir();
    let old = fake(&dir, "Blender 4.2.3 LTS");
    let why = error(&search(None, Some(&dir)));
    assert!(why.contains("is Blender 4.2.3"), "{why}");
    assert!(why.contains(&old.display().to_string()), "{why}");
    assert!(why.contains("Blender 5 or later"), "{why}");
}

#[test]
fn the_override_wins_over_path() {
    let dir = crate::test_temp::dir();
    let on_path = fake(&dir.join("path"), "Blender 4.5.0");
    let chosen = fake(&dir.join("chosen"), "Blender 5.1.0");
    let found = search(Some(&chosen), on_path.parent())
        .find()
        .expect("found");
    assert_eq!((found.path, found.version.as_str()), (chosen, "5.1.0"));
}

#[test]
fn a_place_is_looked_at_after_path() {
    let dir = crate::test_temp::dir();
    let placed = fake(&dir.join("app"), "Blender 5.0.1\n\tbuild date: 2025-12-16");
    let mut search = search(None, Some(&dir.join("empty")));
    search.places = vec![dir.join("missing/blender"), placed.clone()];
    assert_eq!(search.find().expect("found").path, placed);
}

#[test]
fn a_program_that_reports_no_version_is_refused() {
    let dir = crate::test_temp::dir();
    fake(&dir, "hello");
    let why = error(&search(None, Some(&dir)));
    assert!(why.contains("reported no Blender version"), "{why}");
}

#[test]
fn the_version_is_the_word_after_blender() {
    assert_eq!(parse_version("Blender 5.0.1\n").as_deref(), Some("5.0.1"));
    let noisy = "Color management: x\nBlender 5.2.0 Alpha\n\tbuild hash: abc";
    assert_eq!(parse_version(noisy).as_deref(), Some("5.2.0"));
    assert_eq!(parse_version("Blender Foundation\n"), None);
}
