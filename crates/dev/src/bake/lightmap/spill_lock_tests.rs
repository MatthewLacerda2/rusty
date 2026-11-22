use std::fs::File;
use std::path::Path;

use super::spill::{sweep, sweep_one, take_lock, Spill, Taken};

fn project(name: &str) -> (std::path::PathBuf, String) {
    let root = rusty_test_support::temp::dir().join(name);
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join(rusty_core::project::ASSETS_DIR).join("scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    let scene = scenes.join("level.scene").to_string_lossy().into_owned();
    let spills = root
        .join(rusty_core::project::CACHE_DIR)
        .join(super::spill::SPILL_DIR);
    std::fs::create_dir_all(&spills).unwrap();
    (spills, scene)
}

#[test]
fn a_lock_whose_file_a_sweep_removed_before_it_was_taken_is_not_held() {
    let (spills, scene) = project("spill_lock_open");
    let lock = spills.join("level-0-0.lock");
    let taken = take_lock(&lock, || {
        sweep(&scene);
    })
    .unwrap();
    assert!(matches!(taken, Taken::Replaced), "{taken:?}");
    assert!(matches!(take_lock(&lock, || {}).unwrap(), Taken::Held(_)));
}

#[test]
fn a_sweep_whose_lock_file_was_replaced_by_a_live_bake_s_removes_nothing() {
    let (spills, scene) = project("spill_lock_sweep");
    let (dir, lock) = (spills.join("level-0-0"), spills.join("level-0-0.lock"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("0"), b"page").unwrap();
    let mut live: Option<File> = None;
    sweep_one(&dir, &lock, || {
        std::fs::remove_file(&lock).unwrap();
        match take_lock(&lock, || {}).unwrap() {
            Taken::Held(file) => live = Some(file),
            other => panic!("{other:?}"),
        }
    });
    assert!(live.is_some());
    assert!(dir.join("0").is_file(), "the live bake's folder stays");
    assert!(lock.is_file(), "and its lock file");
    drop(live);
    sweep(&scene);
    assert!(!dir.exists() && !lock.exists(), "a dead one is removed");
}

#[test]
fn a_spill_opened_holds_the_lock_file_at_its_path_in_the_project_s_cache() {
    let (spills, scene) = project("spill_lock_held");
    let spill = Spill::open(&scene).unwrap();
    let spills = std::fs::canonicalize(spills).unwrap();
    assert_eq!(spill.dir().parent(), Some(spills.as_path()));
    let mut name = spill.dir().file_name().unwrap().to_os_string();
    name.push(".lock");
    let lock = spill.dir().with_file_name(name);
    assert!(matches!(take_lock(&lock, || {}).unwrap(), Taken::Busy));
    sweep(&scene);
    assert!(spill.dir().is_dir() && Path::new(&lock).is_file());
}
