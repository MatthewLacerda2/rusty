//! `Scene.LoadAsync` through the play loop on the default (inline) loader (#2123):
//! an operation reads ready from the next tick's head and is swapped in at its tail,
//! holds at 0.9 while its activation is not allowed, is replaced by a later load,
//! fails on a bad file with the scene left running, and loads at once in edit mode.

use super::fixture::{logs, world_at};
use super::GameWorld;
use rusty_core::paths;
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// `start.scene` and `next.scene` (one entity each, `Start` / `Next`) in a temp dir.
fn scenes(tag: &str) -> (String, String) {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_2123_async_{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |name: &str| {
        let path = paths::to_slash(&dir.join(format!("{}.scene", name.to_lowercase())));
        let mut scene = Scene::new();
        scene.add_entity(name.into());
        scene.save_to_file(&path).unwrap();
        path
    };
    (write("Start"), write("Next"))
}

fn eval(gw: &GameWorld, code: &str) -> String {
    gw.script_manager().eval(code).unwrap()
}

/// `(status, progress, isDone)` of `op`.
fn state(gw: &GameWorld, op: &str) -> String {
    eval(
        gw,
        &format!(
            "return Scene.GetLoadStatus({op}) .. ' ' .. Scene.GetLoadProgress({op}) \
             .. ' ' .. tostring(Scene.IsLoadDone({op}))"
        ),
    )
}

fn playing(tag: &str) -> (GameWorld, String, String) {
    let (start, next) = scenes(tag);
    let (mut gw, _) = world_at(&start);
    gw.set_playing(true);
    gw.tick(DT);
    (gw, start, next)
}

fn active(gw: &GameWorld) -> String {
    eval(gw, "return Scene.GetActivePath()")
}

#[test]
fn an_operation_is_ready_at_the_next_head_and_swapped_at_its_tail() {
    let (mut gw, _, next) = playing("swap");
    let op = eval(&gw, &format!("return Scene.LoadAsync('{next}')"));
    assert_eq!(state(&gw, &op), "loading 0.0 false");
    gw.tick(DT);
    assert_eq!(state(&gw, &op), "done 1.0 true");
    assert_eq!(active(&gw), next);
    assert!(gw.scene().borrow().find_entity_by_name("Next").is_some());
}

#[test]
fn activation_held_back_waits_at_ready() {
    let (mut gw, start, next) = playing("hold");
    let op = eval(&gw, &format!("return Scene.LoadAsync('{next}')"));
    assert_eq!(
        eval(
            &gw,
            &format!("return Scene.SetAllowSceneActivation({op}, false)")
        ),
        "true"
    );
    for _ in 0..3 {
        gw.tick(DT);
        assert_eq!(state(&gw, &op), "ready 0.9 false");
        assert_eq!(active(&gw), start, "the scene that ran keeps running");
    }
    eval(&gw, &format!("Scene.SetAllowSceneActivation({op}, true)"));
    gw.tick(DT);
    assert_eq!(state(&gw, &op), "done 1.0 true");
    assert_eq!(active(&gw), next);
}

#[test]
fn a_later_load_replaces_a_pending_operation() {
    let (mut gw, start, next) = playing("replace");
    let op = eval(&gw, &format!("return Scene.LoadAsync('{next}')"));
    eval(&gw, &format!("Scene.Load('{start}')"));
    assert_eq!(state(&gw, &op), "replaced 0.0 true");
    gw.tick(DT);
    assert_eq!(active(&gw), start);
    assert!(gw.scene().borrow().find_entity_by_name("Next").is_none());
    let second = eval(&gw, &format!("return Scene.LoadAsync('{next}')"));
    let third = eval(&gw, &format!("return Scene.LoadAsync('{start}')"));
    assert_eq!(state(&gw, &second), "replaced 0.0 true");
    gw.tick(DT);
    assert_eq!(state(&gw, &third), "done 1.0 true");
}

#[test]
fn a_file_that_does_not_parse_fails_and_the_scene_keeps_running() {
    let (start, next) = scenes("bad");
    std::fs::write(&next, "{ not a scene").unwrap();
    let (mut gw, console) = world_at(&start);
    gw.set_playing(true);
    gw.tick(DT);
    let op = eval(&gw, &format!("return Scene.LoadAsync('{next}')"));
    gw.tick(DT);
    assert_eq!(state(&gw, &op), "failed 0.0 true");
    assert_eq!(active(&gw), start);
    let log = logs(&console);
    assert!(
        log.iter().any(|m| m.starts_with("Scene.LoadAsync:")),
        "{log:?}"
    );
}

#[test]
fn a_missing_file_errors_at_the_call() {
    let (gw, _, _) = playing("missing");
    let err = gw
        .script_manager()
        .eval("Scene.LoadAsync('no/such.scene')")
        .unwrap_err();
    assert!(err.contains("Scene.LoadAsync: no scene file at"), "{err}");
    assert!(!gw.scene().borrow().loads.has_pending());
}

#[test]
fn edit_mode_loads_at_once_and_an_unknown_operation_is_gone() {
    let (start, next) = scenes("edit");
    let (mut gw, _) = world_at(&start);
    gw.init_edit_runtime().unwrap();
    let op = eval(&gw, &format!("return Scene.LoadAsync('{next}')"));
    assert_eq!(state(&gw, &op), "done 1.0 true");
    assert_eq!(active(&gw), next);
    assert_eq!(state(&gw, "99"), "gone 0.0 true");
}
