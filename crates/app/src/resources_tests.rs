use super::*;
use rusty_scene::Scene;
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
fn play_frame_reads_the_counter() {
    let mut res = resources();
    assert_eq!(res.play_frame(), 0);
    res.play_frame = 7;
    assert_eq!(res.play_frame(), 7);
}

#[test]
fn flush_storage_is_a_noop_when_pathless() {
    let res = resources();
    res.flush_storage();
    assert!(
        res.console.borrow().messages.is_empty(),
        "pathless flush should not log anything"
    );
}

#[test]
fn flush_storage_logs_on_write_failure() {
    let res = resources();
    let dir = rusty_test_support::temp::dir();
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

#[test]
fn the_ui_api_reads_the_shared_screen_cell() {
    let mut res = resources();
    res.screen.borrow_mut().set_game_view(640, 480);
    res.script_manager.init_runtime().expect("runtime boots");
    let size = res.script_manager.eval("UI.GetScreenSize()").unwrap();
    assert_eq!(size, "640, 480");
    assert_eq!(
        res.ui_view(&rusty_scene::Scene::new()).screen,
        glam::Vec2::new(640.0, 480.0)
    );
}
