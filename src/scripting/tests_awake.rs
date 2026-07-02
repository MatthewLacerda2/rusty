//! Tests for the two-phase script init (#322): all `Awake`s then all `Start`s
//! at play-enter, exactly-once semantics, the disabled-at-load deferral, and
//! the queued load + next-tick init for scripts on entities spawned mid-play.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::components::ScriptComponent;

use super::tests_lifecycle::{manager_with_entity, write_script};

/// Appends one letter per callback (plus the entity id) to a shared global, so
/// tests can assert the exact dispatch order.
const LOGGER: &str = "_G.__log = _G.__log or ''\n\
    return {\n\
    Awake = function(id) __log = __log .. 'A' .. id end,\n\
    Start = function(id) __log = __log .. 'S' .. id end,\n\
    Update = function(id) __log = __log .. 'U' .. id end,\n\
    }";

#[test]
fn init_runs_all_awakes_then_all_starts_exactly_once() {
    let (mut m, a) = manager_with_entity();
    let b = m.scene.borrow_mut().add_entity("E2".to_string());
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script("awake_order", LOGGER);
    m.load_entity_script(a, 0, &path, &BTreeMap::new()).unwrap();
    m.load_entity_script(b, 0, &path, &BTreeMap::new()).unwrap();

    // Play-enter: ALL Awakes (ascending entity order), then ALL Starts.
    m.init_scripts();
    assert_eq!(m.eval("__log").unwrap(), format!("A{a}A{b}S{a}S{b}"));

    // Later ticks: Awake/Start never re-fire; Update follows the init phase.
    m.init_scripts();
    m.update_scripts(0.016);
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("A{a}A{b}S{a}S{b}U{a}U{b}")
    );
}

#[test]
fn disabled_at_load_defers_awake_and_start_until_first_active_tick() {
    let (mut m, id) = manager_with_entity();
    m.scene.borrow_mut().get_entity_mut(id).unwrap().active = false;
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script("awake_disabled", LOGGER);
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();

    // Two full ticks while inactive: no callback fires.
    for _ in 0..2 {
        m.init_scripts();
        m.update_scripts(0.016);
    }
    assert_eq!(m.eval("__log").unwrap(), "");

    // First active tick: Awake, then Start, then Update — all in one tick.
    m.scene.borrow_mut().get_entity_mut(id).unwrap().active = true;
    m.init_scripts();
    m.update_scripts(0.016);
    assert_eq!(m.eval("__log").unwrap(), format!("A{id}S{id}U{id}"));
}

#[test]
fn runtime_spawned_entity_scripts_run_from_the_next_tick() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    m.init_scripts(); // play-enter: nothing attached yet

    // Mid-play spawn: a new entity appears carrying a script — the scene state
    // a `Scene.Instantiate` / `Scene.CreateEntity` from inside another script's
    // Update leaves behind. The rest of the spawn tick must not touch it.
    let path = write_script("awake_spawn", LOGGER);
    let spawned = {
        let mut scene = m.scene.borrow_mut();
        let id = scene.add_entity("Spawned".to_string());
        scene
            .get_entity_mut(id)
            .unwrap()
            .scripts
            .push(ScriptComponent {
                path: path.clone(),
                ..Default::default()
            });
        id
    };
    m.update_scripts(0.016);
    assert_eq!(
        m.eval("__log").unwrap(),
        "nil",
        "script must not load mid-tick"
    );

    // Head of the next tick's script phase: load + Awake + Start, then Update.
    m.init_scripts();
    m.update_scripts(0.016);
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("A{spawned}S{spawned}U{spawned}")
    );
}
