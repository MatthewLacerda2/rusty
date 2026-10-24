//! `play --project` plays the project's startup scene, or `--scene` (#1343): a
//! scenario play-tests the game being built, never the engine's demo scene. The
//! binary runs as a child, since opening a project moves the working directory.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusty::scene::Scene;
use serde_json::Value;

/// A project whose startup scene holds one entity, `Arena`, with a second scene
/// beside it holding `Other`.
fn game() -> PathBuf {
    let root = crate::temp::dir().join("game");
    let scenes = root.join("assets/scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    for (file, name) in [("arena.scene", "Arena"), ("other.scene", "Other")] {
        let mut scene = Scene::new();
        scene.add_entity(name.to_string());
        let path = scenes.join(file);
        scene.save_to_file(&path.to_string_lossy()).unwrap();
    }
    let file = r#"{ "build": { "startup_scene": "assets/scenes/arena.scene" } }"#;
    std::fs::write(root.join("project.rusty"), file).unwrap();
    root
}

/// `play --project <game> [extra…]` on a scenario that steps a few ticks, and its
/// `results.json` when it wrote one.
fn play(game: &Path, extra: &[&str]) -> (Output, Option<Value>) {
    let dir = crate::temp::dir().join("run");
    std::fs::create_dir_all(&dir).unwrap();
    let scenario = dir.join("scenario.lua");
    std::fs::write(&scenario, "Harness.Step(3)\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_play"))
        .arg("--project")
        .arg(game)
        .args(extra)
        .arg(&scenario)
        .arg(&dir)
        .output()
        .expect("play runs");
    let results = std::fs::read_to_string(dir.join("results.json")).ok();
    (out, results.map(|r| serde_json::from_str(&r).unwrap()))
}

/// The names of the entities in a run's final snapshot.
fn names(results: &Value) -> Vec<String> {
    let entities = results["final_snapshot"]["entities"].as_array().unwrap();
    entities
        .iter()
        .map(|e| e["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn play_runs_the_projects_startup_scene() {
    let (out, results) = play(&game(), &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "play passes: {stderr}");
    assert_eq!(
        names(&results.unwrap()),
        ["Arena"],
        "only the startup scene"
    );
}

#[test]
fn scene_flag_overrides_the_startup_scene() {
    let (out, results) = play(&game(), &["--scene", "assets/scenes/other.scene"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "play passes: {stderr}");
    assert_eq!(names(&results.unwrap()), ["Other"]);
}

#[test]
fn a_missing_scene_fails_the_run() {
    let (out, results) = play(&game(), &["--scene=assets/scenes/none.scene"]);
    assert_eq!(out.status.code(), Some(2), "a usage-class failure");
    assert!(
        results.is_none(),
        "no results for a scene that never loaded"
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("none.scene"));
}
