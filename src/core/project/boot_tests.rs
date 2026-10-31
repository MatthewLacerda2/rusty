//! The boot scene's order (#1618): the editor's remembered scene, then the startup
//! scene, then an empty one; a run boots the startup scene whatever is remembered.

use super::{boot_scene, remember_scene};
use crate::core::project::Access;
use std::path::{Path, PathBuf};

const STARTUP: &str = "assets/scenes/race.scene";
const OTHER: &str = "assets/scenes/garage.scene";

fn project(name: &str, scenes: &[&str]) -> PathBuf {
    let root = crate::test_temp::dir().join(name);
    std::fs::remove_dir_all(&root).ok();
    for scene in scenes {
        let path = root.join(crate::core::paths::native(scene));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "{}").unwrap();
    }
    root
}

fn edit(root: &Path) -> String {
    boot_scene(root, STARTUP, Access::Edit)
}

#[test]
fn the_editor_reopens_the_scene_it_remembers() {
    let root = project("remembered", &[STARTUP, OTHER]);
    remember_scene(&root, OTHER).unwrap();
    assert_eq!(edit(&root), OTHER);
}

#[test]
fn without_a_record_the_startup_scene_opens() {
    let root = project("no_record", &[STARTUP]);
    assert_eq!(edit(&root), STARTUP);
}

#[test]
fn a_remembered_scene_that_is_gone_falls_back_to_the_startup_scene() {
    let root = project("gone", &[STARTUP]);
    remember_scene(&root, OTHER).unwrap();
    assert_eq!(edit(&root), STARTUP);
}

#[test]
fn with_neither_scene_on_disk_an_empty_scene_opens() {
    let root = project("empty", &[]);
    remember_scene(&root, OTHER).unwrap();
    assert_eq!(edit(&root), "");
    assert_eq!(boot_scene(&root, "", Access::Edit), "");
}

#[test]
fn a_run_boots_the_startup_scene_whatever_the_editor_remembers() {
    let root = project("run", &[STARTUP, OTHER]);
    remember_scene(&root, OTHER).unwrap();
    assert_eq!(boot_scene(&root, STARTUP, Access::Run), STARTUP);
    let gone = project("run_gone", &[]);
    assert_eq!(boot_scene(&gone, STARTUP, Access::Run), STARTUP);
}

#[test]
fn remembering_overwrites_the_last_record() {
    let root = project("overwrite", &[STARTUP, OTHER]);
    remember_scene(&root, OTHER).unwrap();
    remember_scene(&root, STARTUP).unwrap();
    remember_scene(&root, STARTUP).unwrap();
    assert_eq!(edit(&root), STARTUP);
}
