//! Unit tests for `Resources` (resources.rs): the per-frame dt accessor, the
//! play-frame counter, the debug-path accessor, and the storage-flush boundary
//! (success no-op when pathless, and the error path that logs to the console).
//! Determinism contract: no wall-clock, no RNG.

use super::*;
use crate::scene::Scene;
use std::cell::RefCell;
use std::rc::Rc;

fn resources() -> Resources {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(glam::Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    Resources::new(scene, input, nav, console, camera, time)
}

#[test]
fn dt_reads_the_frame_delta() {
    let mut res = resources();
    assert_eq!(res.dt(), 0.0);
    res.frame_dt = 0.25;
    assert_eq!(res.dt(), 0.25);
}

#[test]
fn play_frame_reads_the_counter() {
    let mut res = resources();
    assert_eq!(res.play_frame(), 0);
    res.play_frame = 7;
    assert_eq!(res.play_frame(), 7);
}

#[test]
fn pathfinding_points_reads_the_debug_path() {
    let mut res = resources();
    assert!(res.pathfinding_points().is_empty());
    res.pathfinding_points = vec![glam::Vec3::X, glam::Vec3::Y];
    assert_eq!(res.pathfinding_points(), &[glam::Vec3::X, glam::Vec3::Y]);
}

#[test]
fn flush_storage_is_a_noop_when_pathless() {
    let res = resources();
    // The default store is pathless (harness/tests), so flush succeeds silently.
    res.flush_storage();
    assert!(
        res.console.borrow().messages.is_empty(),
        "pathless flush should not log anything"
    );
}

#[test]
fn flush_storage_logs_on_write_failure() {
    let res = resources();
    // Bind the store to a path whose parent is a regular file, so create_dir_all and
    // the write both fail — exercising the error branch that logs to the console.
    let dir = std::env::temp_dir();
    let blocker = dir.join(format!("rusty_flush_block_{}", std::process::id()));
    std::fs::write(&blocker, b"x").unwrap();
    let bad_path = blocker.join("nested").join("store.json");
    res.storage.borrow_mut().open(&bad_path).unwrap();
    res.storage
        .borrow_mut()
        .set("ns", "k", serde_json::json!(1));

    res.flush_storage();

    let logged = res
        .console
        .borrow()
        .messages
        .iter()
        .any(|(msg, _)| msg.contains("Failed to flush storage"));
    std::fs::remove_file(&blocker).ok();
    assert!(logged, "a failed flush must log to the console");
}
