//! A script edited outside the engine during Play reaches the playing VM's next
//! instance, while every other change still waits for Stop (#1710).

use super::{waits_for, POLL};
use crate::shell::editor::watch::Watch;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::{Camera, Scene};
use crate::script::scripting::{ConsoleLogs, ScriptManager};
use crate::time::Time;

/// A playing VM over a fresh scene, and that scene.
fn playing_vm() -> (ScriptManager, Rc<RefCell<Scene>>) {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut m = ScriptManager::new(
        Rc::clone(&scene),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -1.0, 1.0, -1.0, 1.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
        Rc::new(RefCell::new(Camera::new(glam::Vec3::ZERO, 0.0, 0.0))),
        Rc::new(RefCell::new(Time::new())),
    );
    m.init_runtime().expect("VM");
    (m, scene)
}

/// Spawn an instance of `path` and return what its top level set `seen` to.
fn spawn(m: &mut ScriptManager, scene: &RefCell<Scene>, path: &str) -> String {
    let id = scene.borrow_mut().add_entity("S".to_string());
    m.load_entity_script(id, 0, path, &BTreeMap::new())
        .expect("loads");
    m.eval("seen").unwrap()
}

#[test]
fn a_script_edited_during_play_reaches_the_next_instance() {
    let dir = crate::test_temp::dir();
    let script = dir.join("enemy.lua");
    let texture = dir.join("texture.png");
    std::fs::write(&script, "seen = 1\nreturn {}").unwrap();
    std::fs::write(&texture, b"red").unwrap();
    let mut watch = Watch::start(&[&dir], POLL).unwrap();
    let (mut m, scene) = playing_vm();
    let path = script.to_string_lossy().into_owned();
    assert_eq!(spawn(&mut m, &scene, &path), "1");
    // The poller reads modification times to the second: move into the next one.
    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&script, "seen = 2\nreturn {}").unwrap();
    std::fs::write(&texture, b"a longer blue").unwrap();
    assert!(waits_for(&mut watch, &script) && waits_for(&mut watch, &texture));
    assert_eq!(watch.apply(true), 1, "only the script is noted during Play");
    let held = texture.to_string_lossy().into_owned();
    assert!(
        watch.pending().contains(&held),
        "the texture waits for Stop"
    );
    assert_eq!(
        spawn(&mut m, &scene, &path),
        "2",
        "the next instance runs the edit"
    );
}
