//! The edit-mode runtime (#1339): the evaluator drives the edit scene outside
//! Play, Stop brings it back after Play's runtime shuts down, and a host that
//! never asked for one (the harness) stays without it.

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use std::cell::RefCell;
use std::rc::Rc;

const DT: f32 = 1.0 / 60.0;

fn world_with(scene: Rc<RefCell<Scene>>) -> GameWorld {
    let nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
    let input = Rc::new(RefCell::new(InputState::new()));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    GameWorld::new(scene, input, Rc::new(RefCell::new(nav)), console)
}

fn edit_world() -> GameWorld {
    let mut gw = world_with(Rc::new(RefCell::new(Scene::new())));
    gw.init_edit_runtime().expect("edit runtime boots");
    gw
}

fn eval(gw: &GameWorld, line: &str) -> String {
    gw.script_manager().eval(line).expect("line evaluates")
}

fn find(gw: &GameWorld, name: &str) -> String {
    eval(gw, &format!("Scene.FindEntityByName(\"{name}\")"))
}

#[test]
fn eval_authors_the_edit_scene_outside_play() {
    let gw = edit_world();
    let id = eval(&gw, "Scene.CreateEntity(\"Probe\", \"Box\")");
    assert_ne!(id, "nil");
    assert!(!gw.scene().borrow().is_playing(), "no Play was entered");
    assert_eq!(
        find(&gw, "Probe"),
        id,
        "the entity is in the live edit scene"
    );
}

#[test]
fn stop_brings_the_edit_runtime_back_on_the_restored_scene() {
    let mut gw = edit_world();
    eval(&gw, "Scene.CreateEntity(\"Kept\")");
    gw.set_playing(true);
    gw.tick(DT);
    eval(&gw, "Scene.CreateEntity(\"PlayOnly\")");
    gw.set_playing(false);
    gw.tick(DT);
    assert!(gw.script_manager().is_live(), "Stop restores the evaluator");
    assert_ne!(find(&gw, "Kept"), "nil", "edit-mode work survives Play");
    assert_eq!(
        find(&gw, "PlayOnly"),
        "nil",
        "it binds the restored edit scene"
    );
}

#[test]
fn a_scene_reloaded_from_disk_is_what_eval_sees() {
    let dir = crate::test_temp::dir().join("rusty_1339_reload");
    std::fs::create_dir_all(&dir).unwrap();
    let path = crate::core::paths::to_slash(&dir.join("disk.scene"));
    let mut on_disk = Scene::new();
    on_disk.add_entity("FromDisk".to_string());
    on_disk.save_to_file(&path).unwrap();
    let gw = edit_world();
    eval(&gw, "Scene.CreateEntity(\"Unsaved\")");
    // What the editor's follow-the-file reload does to the shared cell (#1340).
    gw.scene().borrow_mut().load_from_file(&path).unwrap();
    assert_ne!(find(&gw, "FromDisk"), "nil");
    assert_eq!(find(&gw, "Unsaved"), "nil");
}

#[test]
fn a_host_without_an_edit_runtime_has_none_after_stop() {
    let mut gw = world_with(Rc::new(RefCell::new(Scene::new())));
    gw.set_playing(true);
    gw.tick(DT);
    gw.set_playing(false);
    gw.tick(DT);
    assert!(
        !gw.script_manager().is_live(),
        "the harness's Stop is unchanged"
    );
}
