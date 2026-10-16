//! The picker's decisions (#854): what New makes, what Open accepts, and when it
//! asks before opening a project another engine last opened.

use super::*;
use crate::core::project::{ProjectFile, PROJECT_FILE};

fn fresh(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join("project_picker").join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn project_with_commit(dir: &Path, commit: &str) {
    let file = ProjectFile {
        engine_commit: commit.to_string(),
        ..ProjectFile::default()
    };
    file.write(&dir.join(PROJECT_FILE)).unwrap();
}

#[test]
fn another_engines_project_asks_first_and_cancel_keeps_the_picker() {
    let dir = fresh("mismatch");
    project_with_commit(&dir, "0123456789abcdef0123456789abcdef01234567");
    let warning = check_existing(&dir).unwrap();
    // A build without git can't compare, so it never asks.
    if crate::core::project::engine_commit() == crate::core::project::UNKNOWN_COMMIT {
        assert_eq!(warning, None);
        return;
    }
    assert!(warning.is_some());
    let mut picker = ProjectPicker::new(RecentProjects::default(), None, dir.clone());
    assert_eq!(picker.apply(Action::OpenRecent(dir.clone())), None);
    assert!(picker.confirm.is_some());
    assert_eq!(picker.apply(Action::CancelConfirm), None);
    assert!(picker.confirm.is_none());
    picker.apply(Action::OpenRecent(dir.clone()));
    assert_eq!(picker.apply(Action::ConfirmOpen), Some(dir));
}

#[test]
fn a_matching_project_opens_at_once_and_new_returns_its_folder() {
    let dir = fresh("match");
    project_with_commit(&dir, crate::core::project::engine_commit());
    let mut picker = ProjectPicker::new(RecentProjects::default(), None, dir.clone());
    assert_eq!(
        picker.apply(Action::OpenRecent(dir.clone())),
        Some(dir.clone())
    );
    picker.show_new();
    assert_eq!(
        picker.apply(Action::Create),
        Some(dir.join(DEFAULT_NEW_NAME))
    );
}

#[test]
fn removing_a_recent_project_saves_the_list() {
    let dir = fresh("remove");
    let path = dir.join(recent::RECENT_FILE);
    let mut list = RecentProjects::default();
    list.touch(&dir.join("a"), 1);
    list.touch(&dir.join("b"), 2);
    let mut picker = ProjectPicker::new(list, Some(path.clone()), dir.clone());
    picker.apply(Action::Remove(dir.join("b")));
    let saved = RecentProjects::load(&path);
    assert_eq!(saved.projects.len(), 1);
    assert_eq!(saved.projects[0].path, dir.join("a"));
}

#[test]
fn the_browsers_start_beside_the_latest_project_that_still_exists() {
    let dir = fresh("start");
    std::fs::create_dir_all(dir.join("games/horde")).unwrap();
    let mut list = RecentProjects::default();
    list.touch(&dir.join("games/horde"), 1);
    list.touch(&dir.join("gone/old"), 2);
    let mut picker = ProjectPicker::new(list, None, dir.clone());
    picker.show_open();
    let Page::Open(browser) = &picker.page else {
        panic!("the Open page");
    };
    assert_eq!(browser.dir(), dir.join("games"));
    assert_eq!(browser.subdirs(), ["horde"]);
}
