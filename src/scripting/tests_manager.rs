use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use glam::Vec3;

use crate::core::input::InputState;
use crate::core::storage::Storage;
use crate::core::video::VideoSettings;
use crate::navigation::NavigationGraph;
use crate::render::postfx::QualityPreset;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;

fn make_manager() -> ScriptManager {
    ScriptManager::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
        Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0))),
        Rc::new(RefCell::new(Time::new())),
    )
}

fn live_manager() -> ScriptManager {
    let mut m = make_manager();
    m.init_runtime(&Rc::new(RefCell::new(None))).expect("init");
    m
}

#[test]
fn is_live_false_before_init_true_after() {
    let mut m = make_manager();
    assert!(!m.is_live(), "not live before init_runtime");
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    assert!(m.is_live(), "live after init_runtime");
}

#[test]
fn shutdown_clears_runtime_and_scripts() {
    let mut m = live_manager();
    m.shutdown();
    assert!(!m.is_live(), "lua cleared by shutdown");
    assert!(
        m.entity_scripts.is_empty(),
        "entity_scripts cleared by shutdown"
    );
}

#[test]
fn set_storage_replaces_underlying_cell() {
    let mut m = make_manager();
    let storage = Rc::new(RefCell::new(Storage::new()));
    m.set_storage(Rc::clone(&storage));
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    m.exec("Storage.Set('ns', 'k', 7)").unwrap();
    let val = storage.borrow().get("ns", "k");
    assert_eq!(val.and_then(|v| v.as_f64()), Some(7.0));
}

#[test]
fn quality_cell_roundtrip() {
    let mut m = make_manager();
    let quality = Rc::new(RefCell::new(QualityPreset::High));
    m.set_quality_cell(Rc::clone(&quality));
    let out = m.quality_cell();
    assert!(matches!(*out.borrow(), QualityPreset::High));
    *quality.borrow_mut() = QualityPreset::Low;
    assert!(matches!(*m.quality_cell().borrow(), QualityPreset::Low));
}

#[test]
fn video_cell_roundtrip() {
    let mut m = make_manager();
    let video = Rc::new(RefCell::new(VideoSettings {
        width: 1280,
        height: 720,
        vsync: false,
        fullscreen: false,
    }));
    m.set_video_cell(Rc::clone(&video));
    assert_eq!(m.video_cell().borrow().width, 1280);
    video.borrow_mut().width = 1920;
    assert_eq!(m.video_cell().borrow().width, 1920);
}

#[test]
fn load_entity_script_missing_file_returns_error() {
    let mut m = live_manager();
    let fields = BTreeMap::new();
    let result = m.load_entity_script(1, 0, "/nonexistent/path.lua", &fields);
    assert!(result.is_err(), "missing file must error");
    assert!(result.unwrap_err().contains("Script file not found"));
}
