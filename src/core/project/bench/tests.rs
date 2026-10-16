//! rusty bench's rules (#854, #942): what New makes, what Open accepts, and what each
//! does to the recent list.

use super::*;
use crate::core::project::{engine_commit, ProjectFile, PROJECT_FILE, UNKNOWN_COMMIT};

fn fresh(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join("project_bench").join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    crate::core::paths::canonicalize(&dir).unwrap()
}

#[test]
fn a_new_project_needs_a_plain_name_and_an_empty_folder() {
    let parent = fresh("new");
    assert_eq!(
        new_project_dir(&parent, " Horde "),
        Ok(parent.join("Horde"))
    );
    assert!(new_project_dir(&parent, "  ").is_err());
    assert!(new_project_dir(&parent, ".hidden").is_err());
    assert!(new_project_dir(&parent, "a/b").is_err());
    std::fs::create_dir_all(parent.join("empty")).unwrap();
    assert!(
        new_project_dir(&parent, "empty").is_ok(),
        "an empty folder is fine"
    );
    std::fs::write(parent.join("empty/x"), "").unwrap();
    assert!(new_project_dir(&parent, "empty").is_err());
}

#[test]
fn only_a_project_folder_opens() {
    let dir = fresh("open");
    assert!(check_existing(&dir.join("gone")).is_err());
    let err = check_existing(&dir).unwrap_err();
    assert!(err.contains("not a rusty project"), "{err}");
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    assert_eq!(
        check_existing(&dir),
        Ok(None),
        "a pre-project.rusty project"
    );
}

#[test]
fn create_makes_the_project_and_lists_it_first() {
    let dir = fresh("create");
    let path = dir.join(recent::RECENT_FILE);
    let mut bench = Bench::at(Some(path.clone()));
    let root = bench.create(&dir, "Horde", 7).unwrap();
    assert_eq!(root, dir.join("Horde"));
    assert!(root.join(PROJECT_FILE).is_file() && root.join("assets/scenes").is_dir());
    assert_eq!(RecentProjects::load(&path).projects[0].path, root);
    assert!(
        bench.create(&dir, "Horde", 8).is_err(),
        "not over a project"
    );
}

#[test]
fn open_lists_a_project_and_reports_another_engine() {
    let dir = fresh("bench_open");
    let mut bench = Bench::at(None);
    assert!(bench.open(&dir, 1).is_err(), "not a project");
    assert!(bench.recent.projects.is_empty());
    let file = ProjectFile {
        engine_commit: "0123456789abcdef0123456789abcdef01234567".to_string(),
        ..ProjectFile::default()
    };
    file.write(&dir.join(PROJECT_FILE)).unwrap();
    let (root, warning) = bench.open(&dir.join("."), 2).unwrap();
    assert_eq!(root, dir, "listed resolved");
    assert_eq!(warning.is_some(), engine_commit() != UNKNOWN_COMMIT);
    assert_eq!(bench.recent.projects[0].path, dir);
}

#[test]
fn forget_takes_the_entry_off_and_leaves_the_folder() {
    let dir = fresh("forget");
    let path = dir.join(recent::RECENT_FILE);
    let mut bench = Bench::at(Some(path.clone()));
    bench.recent.touch(&dir, 1);
    bench.recent.touch(&dir.join("gone"), 2);
    assert_eq!(
        bench.forget(&dir.join("gone")),
        Ok(true),
        "a missing folder"
    );
    assert_eq!(bench.forget(&dir.join(".")), Ok(true), "by any spelling");
    assert_eq!(bench.forget(&dir), Ok(false));
    assert!(RecentProjects::load(&path).projects.is_empty());
    assert!(dir.is_dir());
}
