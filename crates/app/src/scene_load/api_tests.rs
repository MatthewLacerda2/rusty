//! `Scene.Load` determinism and edit-mode semantics (#432): a replay across a load is
//! byte-identical, an edit-mode load is File ▸ Open (immediate), and a bad path
//! errors with the path in the message.

use super::fixture::{logs, world_at, write_scenes};

const DT: f32 = 1.0 / 60.0;

/// Play `menu` for 20 ticks (the load lands on tick 1); the console transcript and
/// the final scene document.
fn run(menu: &str) -> (Vec<String>, String) {
    let (mut gw, console) = world_at(menu);
    gw.set_playing(true);
    for _ in 0..20 {
        gw.tick(DT);
    }
    let doc = rusty_scene::to_scene_data(&gw.scene().borrow());
    (logs(&console), serde_json::to_string(&doc).unwrap())
}

#[test]
fn a_replay_across_a_load_is_identical() {
    let (menu, _) = write_scenes("replay");
    let first = run(&menu);
    assert!(first.0.iter().any(|m| m.starts_with("level:start")));
    assert_eq!(first, run(&menu));
}

#[test]
fn edit_mode_load_is_immediate_and_sets_the_scene_file() {
    let (menu, level) = write_scenes("edit");
    let (mut gw, _) = world_at(&menu);
    gw.init_edit_runtime().unwrap();
    let before = gw.scene().borrow().id();

    gw.script_manager()
        .eval(&format!("Scene.Load('{level}')"))
        .unwrap();
    let scene = gw.scene().borrow();
    assert!(scene.find_entity_by_name("Level").is_some());
    assert!(scene.find_entity_by_name("Menu").is_none());
    assert_ne!(scene.id(), before, "a loaded scene is a new scene");
    drop(scene);
    let path = gw.script_manager().scene_path_cell().borrow().clone();
    assert_eq!(path.as_deref(), Some(level.as_str()));

    let err = gw.script_manager().eval("Scene.DontDestroyOnLoad(1)");
    assert!(err.unwrap_err().contains("only works during play"));
}

#[test]
fn a_missing_scene_file_errors_with_its_path() {
    let (menu, _) = write_scenes("missing");
    let (mut gw, _) = world_at(&menu);
    gw.init_edit_runtime().unwrap();
    let call = "Scene.Load('no/such/level.scene')";
    let edit = gw.script_manager().eval(call).unwrap_err();
    assert!(edit.contains("no/such/level.scene"), "{edit}");

    gw.set_playing(true);
    gw.tick(DT);
    let play = gw.script_manager().eval(call).unwrap_err();
    assert!(play.contains("no/such/level.scene"), "{play}");
    assert!(
        !gw.scene().borrow().loads.has_pending(),
        "nothing was queued"
    );
}

/// A play-mode load drops the environment queries in flight (#2116): an answer
/// made against the outgoing scene, unread, reads as gone after it, even one
/// whose querier survives the load.
#[test]
fn a_load_drops_the_environment_queries_in_flight() {
    let (_, level) = write_scenes("queries");
    let (mut gw, _) = world_at(&level);
    gw.set_playing(true);
    gw.tick(DT);
    let ask = "local q = Scene.FindEntityByName('Level'); Scene.DontDestroyOnLoad(q) \
               ticket = EnvironmentQuery.Request(q, { generator = { points = { { 0, 0, 0 } } } })";
    gw.script_manager().eval(ask).unwrap();
    let poll = "return (EnvironmentQuery.Poll(ticket))";
    gw.script_manager()
        .eval(&format!("Scene.Load('{level}')"))
        .unwrap();
    gw.tick(DT);
    assert_eq!(gw.script_manager().eval(poll).unwrap(), "gone");
}
