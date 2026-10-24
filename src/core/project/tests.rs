//! The project seam (#829): the `--project` flag, the lookups and the skeleton.
//! `open` itself moves the working directory, so it is exercised by the binaries,
//! not here.

use super::locate::{find_engine_dir, find_packaged_project};
use super::*;
use crate::core::paths::native;

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
    assert_eq!(take_flag(&mut a), Ok(Some(native("../game"))));
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
fn any_flag_takes_its_value_and_an_empty_one_is_an_error() {
    let mut a = args(&["a.lua", "--scene", "assets/scenes/x.scene", "out"]);
    let taken = take_value(&mut a, "--scene");
    assert_eq!(taken, Ok(Some("assets/scenes/x.scene".into())));
    assert_eq!(a, args(&["a.lua", "out"]));
    assert_eq!(
        take_value(&mut args(&["--scene=s"]), "--scene"),
        Ok(Some("s".into()))
    );
    assert_eq!(take_value(&mut args(&["--scenery"]), "--scene"), Ok(None));
    assert!(
        take_value(&mut args(&["--scene="]), "--scene").is_err(),
        "empty"
    );
    assert!(take_flag(&mut args(&["--project="])).is_err(), "empty");
}

#[test]
fn a_named_project_wins_and_the_default_is_dot_project() {
    assert_eq!(locate(Some("g".into())), PathBuf::from("g"));
    // A test binary has no packaged project beside it.
    assert_eq!(locate(None), PathBuf::from(DEFAULT_PROJECT_DIR));
}

#[test]
fn the_engine_content_is_found_from_this_checkout() {
    assert!(engine_dir().join("shaders").join("common.wgsl").is_file());
    assert!(engine_path("shaders/common.wgsl").ends_with("engine/shaders/common.wgsl"));
}

#[test]
fn a_shipped_layout_finds_its_engine_and_project_beside_the_executable() {
    let ship = fresh("ship");
    std::fs::create_dir_all(ship.join("engine").join("shaders")).unwrap();
    std::fs::write(
        ship.join(crate::core::paths::native("engine/shaders/common.wgsl")),
        "",
    )
    .unwrap();
    std::fs::create_dir_all(ship.join("project").join("assets")).unwrap();
    assert_eq!(find_engine_dir(&ship), Some(ship.join("engine")));
    assert_eq!(find_packaged_project(&ship), Some(ship.join("project")));

    // A macOS bundle: Contents/MacOS/<exe>, Contents/Resources/{engine,project}.
    let bundle = fresh("bundle").join("Game.app").join("Contents");
    let (macos, res) = (bundle.join("MacOS"), bundle.join("Resources"));
    std::fs::create_dir_all(&macos).unwrap();
    std::fs::create_dir_all(res.join("engine").join("shaders")).unwrap();
    std::fs::write(
        res.join(crate::core::paths::native("engine/shaders/common.wgsl")),
        "",
    )
    .unwrap();
    std::fs::create_dir_all(res.join("project").join("assets")).unwrap();
    let found = find_engine_dir(&macos).unwrap();
    assert!(
        found.join("shaders").join("common.wgsl").is_file(),
        "{found:?}"
    );
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
    assert!(!root.join(".gitignore").exists(), "no root .gitignore");
}
