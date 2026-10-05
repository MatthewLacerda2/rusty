//! The project seam (#829): the `--project` flag, the lookups, the skeleton and the
//! one-time legacy migration. `open` itself moves the working directory, so it is
//! exercised by the binaries, not here.

use super::locate::{find_engine_dir, find_packaged_project};
use super::migrate::{legacy_layout, rewrite};
use super::*;

fn fresh(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join("project_seam").join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

#[test]
fn the_project_flag_is_taken_out_of_the_arguments() {
    let mut a = args(&["scene.scene", "--project", "../game", "--empty"]);
    assert_eq!(take_flag(&mut a), Ok(Some(PathBuf::from("../game"))));
    assert_eq!(a, args(&["scene.scene", "--empty"]));
    let mut b = args(&["--project=/abs/game"]);
    assert_eq!(take_flag(&mut b), Ok(Some(PathBuf::from("/abs/game"))));
    assert!(b.is_empty());
    let mut none = args(&["x"]);
    assert_eq!(take_flag(&mut none), Ok(None));
    assert!(take_flag(&mut args(&["--project"])).is_err(), "no value");
    assert!(take_flag(&mut args(&["--project", "--empty"])).is_err());
}

#[test]
fn a_named_project_wins_and_the_default_is_dot_project() {
    assert_eq!(locate(Some("g".into())), PathBuf::from("g"));
    // A test binary has no packaged project beside it.
    assert_eq!(locate(None), PathBuf::from(DEFAULT_PROJECT_DIR));
}

#[test]
fn the_engine_content_is_found_from_this_checkout() {
    assert!(engine_dir().join("shaders/common.wgsl").is_file());
    assert!(engine_path("shaders/common.wgsl").ends_with("engine/shaders/common.wgsl"));
}

#[test]
fn a_shipped_layout_finds_its_engine_and_project_beside_the_executable() {
    let ship = fresh("ship");
    std::fs::create_dir_all(ship.join("engine/shaders")).unwrap();
    std::fs::write(ship.join("engine/shaders/common.wgsl"), "").unwrap();
    std::fs::create_dir_all(ship.join("project/assets")).unwrap();
    assert_eq!(find_engine_dir(&ship), Some(ship.join("engine")));
    assert_eq!(find_packaged_project(&ship), Some(ship.join("project")));

    // A macOS bundle: Contents/MacOS/<exe>, Contents/Resources/{engine,project}.
    let bundle = fresh("bundle").join("Game.app/Contents");
    let (macos, res) = (bundle.join("MacOS"), bundle.join("Resources"));
    std::fs::create_dir_all(&macos).unwrap();
    std::fs::create_dir_all(res.join("engine/shaders")).unwrap();
    std::fs::write(res.join("engine/shaders/common.wgsl"), "").unwrap();
    std::fs::create_dir_all(res.join("project/assets")).unwrap();
    let found = find_engine_dir(&macos).unwrap();
    assert!(found.join("shaders/common.wgsl").is_file(), "{found:?}");
    assert!(find_packaged_project(&macos).is_some());
    assert_eq!(find_packaged_project(&fresh("bare")), None);
}

#[test]
fn the_skeleton_is_created_and_its_derived_folders_ignore_themselves() {
    let root = fresh("skeleton");
    create_skeleton(&root).unwrap();
    create_skeleton(&root).unwrap();
    for sub in SKELETON {
        assert!(root.join(sub).is_dir(), "{sub}");
    }
    let ignore = std::fs::read_to_string(root.join(CACHE_DIR).join(".gitignore")).unwrap();
    assert_eq!(ignore, "*\n");
    assert!(!root.join(".gitignore").exists(), "the root is the user's");
}

#[test]
fn legacy_paths_become_project_relative() {
    let scene = r#"{"path": "project/assets/scripts/bot.lua", "s": "project/scenes/a.scene"}"#;
    assert_eq!(
        rewrite(scene).unwrap(),
        r#"{"path": "assets/scripts/bot.lua", "s": "assets/scenes/a.scene"}"#
    );
    assert_eq!(
        rewrite("00ff project/scenarios/x.lua\n").unwrap(),
        "00ff scenarios/x.lua\n"
    );
    assert_eq!(
        rewrite("Scene.Instantiate('project/prefabs/E.prefab')").unwrap(),
        "Scene.Instantiate('assets/prefabs/E.prefab')"
    );
    assert_eq!(rewrite("-- see myproject/notes and \"assets/x\""), None);
}

#[test]
fn a_legacy_project_is_moved_and_rewritten_once() {
    let root = fresh("legacy");
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write(
        "build_settings.json",
        r#"{"startup_scene": "project/scenes/default.scene"}"#,
    );
    write(
        "scenes/default.scene",
        r#"{"path": "project/assets/scripts/bot.lua"}"#,
    );
    write("assets/scripts/bot.lua", "return {}");
    write(".seeded", "00ff project/assets/scripts/bot.lua\n");
    write("storage.json", "{}");
    create_skeleton(&root).unwrap();

    let lines = legacy_layout(&root).unwrap();
    assert!(
        lines.iter().any(|l| l.contains("moved scenes/")),
        "{lines:?}"
    );
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
    assert_eq!(
        read("build_settings.json"),
        r#"{"startup_scene": "assets/scenes/default.scene"}"#
    );
    assert_eq!(
        read("assets/scenes/default.scene"),
        r#"{"path": "assets/scripts/bot.lua"}"#
    );
    assert_eq!(read(".seeded"), "00ff assets/scripts/bot.lua\n");
    assert!(root.join("saved/storage.json").is_file());
    assert!(!root.join("scenes").exists());
    assert!(
        legacy_layout(&root).unwrap().is_empty(),
        "a second open changes nothing"
    );
}
