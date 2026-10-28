//! `Scene.SetStatic` / `Scene.IsStatic` (#1496): the inspector's Static checkbox
//! from Lua, writing the same `World::set_static` flag the bakes read.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

fn game() -> (GameWorld, Rc<RefCell<Scene>>, u32) {
    let mut s = Scene::new();
    let id = s.add_entity("Floor".to_string());
    let scene = Rc::new(RefCell::new(s));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut game = crate::play::world(scene.clone(), 1.0, console);
    game.set_playing(true);
    game.tick(1.0 / 60.0);
    (game, scene, id)
}

#[test]
fn set_static_writes_the_flag_the_inspector_writes() {
    let (game, scene, id) = game();
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    assert_eq!(eval(&format!("return Scene.IsStatic({id})")), "false");
    assert_eq!(eval(&format!("return Scene.SetStatic({id}, true)")), "true");
    assert!(scene.borrow().world.is_static(id));
    assert_eq!(eval(&format!("return Scene.IsStatic({id})")), "true");
    assert_eq!(
        eval(&format!("return Scene.SetStatic({id}, false)")),
        "true"
    );
    assert!(!scene.borrow().world.is_static(id));
}

#[test]
fn a_missing_or_nil_id_reads_false_and_a_nil_write_raises() {
    let (game, _, _) = game();
    let sm = game.script_manager();
    assert_eq!(
        sm.eval("return Scene.SetStatic(99999, true)").unwrap(),
        "false"
    );
    assert_eq!(sm.eval("return Scene.IsStatic(99999)").unwrap(), "false");
    assert_eq!(sm.eval("return Scene.IsStatic(nil)").unwrap(), "false");
    let e = sm.eval("Scene.SetStatic(nil, true)").expect_err("raises");
    assert!(e.contains("Scene.SetStatic: no entity (nil id)"), "{e}");
}
