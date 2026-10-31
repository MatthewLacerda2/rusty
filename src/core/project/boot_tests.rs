//! The boot scene's order (#1618): the editor's remembered scene, then the startup
//! scene, then an empty one; a run boots the startup scene whatever is remembered.

use super::{boot_scene, remember_scene, scene_to_remember};
use crate::core::project::Access;
use std::path::{Path, PathBuf};

const STARTUP: &str = "assets/scenes/race.scene";
const OTHER: &str = "assets/scenes/garage.scene";

fn project(name: &str, scenes: &[&str]) -> PathBuf {
    let root = crate::test_temp::dir().join("boot_scene").join(name);
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    for scene in scenes {
        let path = root.join(crate::core::paths::native(scene));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "{}").unwrap();
    }
    root
}

fn edit(root: &Path) -> Vec<String> {
    boot_scene(root, STARTUP, Access::Edit).unwrap().scenes
}

#[test]
fn the_editor_reopens_the_scene_it_remembers_then_falls_back_to_the_startup_one() {
    let root = project("remembered", &[STARTUP, OTHER]);
    remember_scene(&root, OTHER).unwrap();
    assert_eq!(edit(&root), [OTHER, STARTUP]);
}

#[test]
fn without_a_record_the_startup_scene_opens() {
    let root = project("no_record", &[STARTUP]);
    assert_eq!(edit(&root), [STARTUP]);
    remember_scene(&root, STARTUP).unwrap();
    assert_eq!(edit(&root), [STARTUP], "one scene is tried once");
}

#[test]
fn a_remembered_scene_that_is_gone_falls_back_to_the_startup_scene() {
    let root = project("gone", &[STARTUP]);
    remember_scene(&root, OTHER).unwrap();
    assert_eq!(edit(&root), [STARTUP]);
}

#[test]
fn with_neither_scene_on_disk_an_empty_scene_opens_and_says_why() {
    let root = project("empty", &[]);
    remember_scene(&root, OTHER).unwrap();
    let plan = boot_scene(&root, STARTUP, Access::Edit).unwrap();
    assert!(plan.scenes.is_empty());
    assert_eq!(
        plan.warnings.len(),
        1,
        "a named startup scene with no file warns"
    );
    let unset = boot_scene(&root, "", Access::Edit).unwrap();
    assert_eq!(
        unset,
        Default::default(),
        "no startup scene named: nothing to say"
    );
}

#[test]
fn a_run_boots_the_startup_scene_whatever_the_editor_remembers() {
    let root = project("run", &[STARTUP, OTHER]);
    remember_scene(&root, OTHER).unwrap();
    let run = |root: &Path, startup| boot_scene(root, startup, Access::Run);
    assert_eq!(run(&root, STARTUP).unwrap().scenes, [STARTUP]);
    let gone = project("run_gone", &[]);
    assert_eq!(
        run(&gone, STARTUP).unwrap().scenes,
        [STARTUP],
        "named, so it fails loudly"
    );
    assert!(
        run(&root, "").is_err(),
        "a run with no startup scene is an error"
    );
    assert!(run(&root, "  ").is_err());
}

#[test]
fn remembering_overwrites_the_last_record() {
    let root = project("overwrite", &[STARTUP, OTHER]);
    remember_scene(&root, OTHER).unwrap();
    remember_scene(&root, STARTUP).unwrap();
    remember_scene(&root, STARTUP).unwrap();
    assert_eq!(edit(&root), [STARTUP]);
}

#[test]
fn the_editor_remembers_a_project_scene_by_its_relative_path() {
    let root = project("remember", &[OTHER]);
    let absolute = root.join(crate::core::paths::native(OTHER));
    let absolute = absolute.to_string_lossy();
    let remember = |open, last, playing| scene_to_remember(&root, open, last, playing);
    assert_eq!(remember(Some(OTHER), None, false).as_deref(), Some(OTHER));
    assert_eq!(
        remember(Some(&*absolute), None, false).as_deref(),
        Some(OTHER)
    );
    assert_eq!(
        remember(Some("./assets\\x.scene"), None, false).as_deref(),
        Some("assets/x.scene")
    );
    assert_eq!(
        remember(None, Some(OTHER), false),
        None,
        "an Untitled scene"
    );
    assert_eq!(
        remember(Some(OTHER), None, true),
        None,
        "a scene in Play is the game's"
    );
    assert_eq!(
        remember(Some(OTHER), Some(OTHER), false),
        None,
        "already recorded"
    );
    assert_eq!(
        remember(Some("../elsewhere.scene"), None, false),
        None,
        "outside"
    );
    let outside = crate::test_temp::dir().join("not_this_project.scene");
    let outside = outside.to_string_lossy();
    assert_eq!(
        remember(Some(&*outside), None, false),
        None,
        "outside, absolute"
    );
}
