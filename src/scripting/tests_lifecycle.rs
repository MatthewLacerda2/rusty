use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;

fn manager_with_entity() -> (ScriptManager, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("E".to_string());
    let m = ScriptManager::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(-5.0, 5.0, -5.0, 5.0, 1.0))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
        Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
        Rc::new(RefCell::new(Time::new())),
    );
    (m, id)
}

fn write_script(name: &str, code: &str) -> String {
    let path = format!("/tmp/rusty_lifecycle_{}.lua", name);
    std::fs::write(&path, code).unwrap();
    path
}

#[test]
fn eval_expression_returns_value_string() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    assert_eq!(m.eval("1 + 1").unwrap(), "2");
    assert_eq!(m.eval("'hello'").unwrap(), "hello");
    assert_eq!(m.eval("true").unwrap(), "true");
    assert_eq!(m.eval("nil").unwrap(), "nil");
    assert_eq!(m.eval("{}").unwrap(), "<table>");
    assert_eq!(m.eval("function() end").unwrap(), "<function>");
}

#[test]
fn eval_empty_returns_empty_string() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    assert_eq!(m.eval("").unwrap(), "");
    assert_eq!(m.eval("   ").unwrap(), "");
}

#[test]
fn eval_statement_is_accepted() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    m.eval("x = 99").unwrap();
    assert_eq!(m.eval("x").unwrap(), "99");
}

#[test]
fn eval_lua_error_propagates_as_err() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    assert!(m.eval("error('boom')").is_err());
}

#[test]
fn start_scripts_invokes_start_for_each_entity() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script("start", &format!(
        "_G.__start_id = 0\nreturn {{ Start = function(id) _G.__start_id = id end }}"
    ));
    m.load_entity_script(id, 0, &path, &BTreeMap::new()).unwrap();
    m.start_scripts();
    assert_eq!(m.eval("__start_id").unwrap(), id.to_string());
}

#[test]
fn update_scripts_passes_delta_time_to_update() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script("update", &format!(
        "_G.__update_dt = 0\nreturn {{ Update = function(id, dt) _G.__update_dt = dt end }}"
    ));
    m.load_entity_script(id, 0, &path, &BTreeMap::new()).unwrap();
    m.update_scripts(0.25);
    let got: f64 = m.eval("__update_dt").unwrap().parse().unwrap();
    assert!((got - 0.25).abs() < 1e-6, "dt should be 0.25, got {got}");
}

#[test]
fn dispatch_trigger_events_calls_on_trigger() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script("trigger", &format!(
        "_G.__other = 0\nreturn {{ OnTrigger = function(id, other) _G.__other = other end }}"
    ));
    m.load_entity_script(id, 0, &path, &BTreeMap::new()).unwrap();
    m.dispatch_trigger_events(vec![(id, 99)]);
    assert_eq!(m.eval("__other").unwrap(), "99");
}
