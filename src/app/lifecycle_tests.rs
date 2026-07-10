//! End-to-end proof of the #323 transition callbacks through the real
//! `GameWorld` play loop (not the dispatch helper in isolation): a script that
//! destroys its own entity during `Update` sees `OnDisable` then `OnDestroy`
//! fire and the entity gone within the same tick, because `apply_destroys` is
//! registered at the tick's tail; and Stop (play-exit) fires neither — the edit
//! snapshot restore is a reset, not gameplay. Teardown callbacks `print` to the
//! shared console, which survives the runtime shutdown Stop performs, so the
//! log is where we read what fired.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::{Scene, ScriptComponent};
use crate::scripting::ConsoleLogs;

use super::GameWorld;

const DT: f32 = 1.0 / 60.0;

/// A play-ready world with one scripted entity, plus a clone of the shared
/// console so a test can read what the script's callbacks logged. `tag` keeps
/// each test's temp script file distinct — the tests run in parallel threads,
/// and a shared path is a write/load race.
fn world_with_script(tag: &str, code: &str) -> (GameWorld, Rc<RefCell<ConsoleLogs>>, u32) {
    let script = std::env::temp_dir().join(format!("rusty_323_lifecycle_{tag}.lua"));
    std::fs::write(&script, code).unwrap();

    let mut s = Scene::new();
    let id = s.add_entity("Scripted".to_string());
    *s.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: script.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];

    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut gw = GameWorld::new(
        Rc::new(RefCell::new(s)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -20.0, 20.0, -20.0, 20.0, 1.0,
        ))),
        Rc::clone(&console),
    );
    gw.set_playing(true);
    (gw, console, id)
}

/// Index of the first console message equal to `msg`, if logged.
fn log_index(console: &Rc<RefCell<ConsoleLogs>>, msg: &str) -> Option<usize> {
    console.borrow().messages.iter().position(|(m, _)| m == msg)
}

#[test]
fn destroy_during_play_fires_disable_then_destroy_and_removes_entity() {
    // The script destroys itself on its first Update; its teardown callbacks log.
    let (mut gw, console, id) = world_with_script(
        "destroy",
        "return {\n\
         Update = function(id) Scene.DestroyEntity(id) end,\n\
         OnDisable = function(id) print('disable') end,\n\
         OnDestroy = function(id) print('destroy') end,\n\
         }",
    );

    gw.tick(DT); // Awake/OnEnable/Start → Update (requests destroy) → apply_destroys

    let disable = log_index(&console, "disable");
    let destroy = log_index(&console, "destroy");
    assert!(
        disable.is_some() && destroy.is_some(),
        "both OnDisable and OnDestroy fired on a play-mode destroy"
    );
    assert!(
        disable < destroy,
        "OnDisable fires immediately before OnDestroy"
    );
    assert!(
        !gw.scene().borrow().world.contains(id),
        "the destroyed entity is removed within the same tick"
    );
}

#[test]
fn stop_fires_no_teardown_callbacks() {
    // Same teardown-logging script, but this time we Stop instead of destroying.
    let (mut gw, console, id) = world_with_script(
        "stop",
        "return {\n\
         OnDisable = function(id) print('disable') end,\n\
         OnDestroy = function(id) print('destroy') end,\n\
         }",
    );

    gw.tick(DT); // enter play + one tick: Awake/OnEnable/Start, no teardown
    assert!(
        log_index(&console, "disable").is_none() && log_index(&console, "destroy").is_none(),
        "no teardown callback fires during normal play"
    );

    gw.set_playing(false);
    gw.poll_transition(); // exit_play: shutdown + restore the edit snapshot

    assert!(
        log_index(&console, "disable").is_none() && log_index(&console, "destroy").is_none(),
        "Stop fires neither OnDisable nor OnDestroy — the snapshot restore is a reset"
    );
    assert!(
        gw.scene().borrow().world.contains(id),
        "Stop restores the edit scene, so the entity is back (not destroyed)"
    );
}
