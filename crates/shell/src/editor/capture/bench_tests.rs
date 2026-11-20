//! The editor round end to end (#2314), on a primitive project written to a temp
//! dir: a floor, a camera and a light probe. Renders when an adapter is there
//! (hence `gpu_`); without one the round answers `None` and makes no rows up.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_scene::create::{create_entity, Primitive};

use super::*;

/// The project's root and its one scene's path.
fn project() -> (PathBuf, String) {
    let root = rusty_test_support::temp::dir().join(format!("editor_round_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join("assets/scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    let mut yard = Scene::new();
    let floor = create_entity(&mut yard, "Floor", Some(Primitive::Box));
    yard.world.transform_mut(floor).unwrap().scale = Vec3::new(10.0, 0.2, 10.0);
    let cam = yard.add_entity("Cam".into());
    yard.world.set_camera(cam, Some(CameraComponent::default()));
    yard.world
        .set_world_position(cam, Vec3::new(0.0, 2.0, -4.0));
    yard.probes.add_probe(Vec3::new(0.0, 1.0, 0.0));
    let path = scenes.join("yard.scene").to_string_lossy().into_owned();
    rusty_edit::save_to_file(&yard, &path).unwrap();
    (root, path)
}

/// Every file under `dir`, by path, with its bytes.
fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut dirs = vec![dir.to_path_buf()];
    while let Some(d) = dirs.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                out.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

#[test]
fn gpu_the_editor_round_measures_frames_play_stop_and_save_and_writes_nothing() {
    let (root, scene) = project();
    let before = files(&root);
    let save_dir =
        rusty_test_support::temp::dir().join(format!("editor_save_{}", std::process::id()));
    let opts = Round {
        size: (160, 90),
        warmup: 1,
        frames: 2,
        runs: 2,
        warm: false,
        save_dir: save_dir.clone(),
    };
    let mut game = editor_world(&scene, ProjectSettings::default()).unwrap();
    assert!(
        !game.resources.application.borrow().standalone(),
        "Play takes its snapshot"
    );
    let Some(round) = round(&mut game, &opts).unwrap() else {
        return; // no adapter: nothing measured, nothing made up
    };
    let row = |m: &str| round.rows.iter().find(|r| r.metric == m);
    for metric in ["editor.ui_ms", "play_enter_ms", "play_stop_ms", "save_ms"] {
        let found = row(metric);
        assert!(
            found.is_some_and(|r| r.avg > 0.0),
            "{metric}: {:?}",
            round.rows
        );
    }
    for metric in ["editor.render.handles", "editor.render.outline"] {
        assert!(row(metric).is_some(), "{metric}: {:?}", round.rows);
    }
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        assert!(row("play_rss_mb").is_some(), "{:?}", round.rows);
    }
    // Two runs: each single sample has its run-to-run spread.
    let enter = row("play_enter_ms").unwrap();
    assert!(enter.lo <= enter.avg && enter.avg <= enter.hi, "{enter:?}");
    assert!(
        !game.scene().borrow().is_playing(),
        "the round ends in edit mode"
    );
    assert_eq!(files(&root), before, "the project is read-only");
    assert!(!save_dir.exists(), "the save is deleted after");
}
