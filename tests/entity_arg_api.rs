//! A `nil` entity id across the API (#1383): every read answers what it answers
//! for a missing entity, and every write raises an error naming its verb, never a
//! `u32` conversion message. One `api::EntityArg` decides it for every binding.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

fn game() -> GameWorld {
    let mut s = Scene::new();
    s.add_entity("Thing".to_string());
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut game = crate::play::world(Rc::new(RefCell::new(s)), 1.0, console);
    game.set_playing(true);
    game.tick(1.0 / 60.0);
    game
}

#[test]
fn a_nil_id_reads_as_a_missing_entity() {
    let game = game();
    let eval = |line: &str| game.script_manager().eval(line).expect("evaluates");
    let gone = "Scene.FindEntityByName('Gone')";
    assert_eq!(
        eval(&format!("return Transform.GetPosition({gone})")),
        "0, 0, 0"
    );
    assert_eq!(
        eval(&format!("return Transform.GetPosition(99999)")),
        "0, 0, 0"
    );
    assert_eq!(eval("return Transform.GetLocalScale(nil)"), "1, 1, 1");
    assert_eq!(eval("return Layers.GetLayer(nil)"), "0");
    assert_eq!(
        eval("return Light.GetIntensity(nil)"),
        eval("return Light.GetIntensity(99999)")
    );
    assert_eq!(eval("return Scene.IsActive(nil)"), "false");
}

#[test]
fn a_nil_id_on_a_write_is_an_error_naming_the_verb() {
    let game = game();
    let err = |line: &str| game.script_manager().eval(line).expect_err("raises");
    for (line, verb) in [
        ("Scene.SetActive(nil, true)", "Scene.SetActive"),
        (
            "Transform.SetPosition(nil, 1, 2, 3)",
            "Transform.SetPosition",
        ),
        ("Layers.SetLayer(nil, 3)", "Layers.SetLayer"),
        ("Scene.DontDestroyOnLoad(nil)", "Scene.DontDestroyOnLoad"),
    ] {
        let e = err(line);
        assert!(
            e.contains(&format!("{verb}: no entity (nil id)")),
            "{line}: {e}"
        );
        assert!(!e.contains("u32"), "{line}: {e}");
    }
    // A dead id on a write stays the no-op it always was.
    let ok = game
        .script_manager()
        .eval("Transform.SetPosition(99999, 1, 2, 3)");
    assert!(ok.is_ok(), "{ok:?}");
}
