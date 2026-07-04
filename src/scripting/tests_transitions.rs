//! Tests for the transition callbacks (#323): `OnEnable`/`OnDisable` on the
//! entity's `active` edges, `OnDestroy` (preceded by `OnDisable` when active) on
//! the deferred destroy path, the once-per-edge diff, the unified active gate
//! (a disabled entity gets no `OnTrigger`), and deterministic dispatch order.
//! Each callback tags a shared string: `A`wake, `E`nable, `S`tart, `U`pdate,
//! `D`isable, destro`X` — so one log asserts the exact order.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::physics::TriggerEvents;

use super::tests_lifecycle::{manager_with_entity, write_script};

const DT: f32 = 1.0 / 60.0;

const LOGGER: &str = "_G.__log = _G.__log or ''\n\
    return {\n\
    Awake = function(id) __log = __log .. 'A' .. id end,\n\
    OnEnable = function(id) __log = __log .. 'E' .. id end,\n\
    Start = function(id) __log = __log .. 'S' .. id end,\n\
    Update = function(id) __log = __log .. 'U' .. id end,\n\
    OnDisable = function(id) __log = __log .. 'D' .. id end,\n\
    OnDestroy = function(id) __log = __log .. 'X' .. id end,\n\
    }";

/// A manager whose sole entity carries the tagging `LOGGER` script (slot 0).
/// `name` must be unique per test: `write_script` keys the temp path on it, so a
/// shared name would race the parallel test writer against another's reader.
fn logging_manager(name: &str) -> (super::manager::ScriptManager, u32) {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(name, LOGGER);
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    (m, id)
}

#[test]
fn first_activation_orders_awake_enable_start() {
    let (mut m, id) = logging_manager("trans_first");
    m.init_scripts();
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("A{id}E{id}S{id}"),
        "first active tick fires Awake, then OnEnable, then Start"
    );
}

#[test]
fn disable_then_reenable_fire_each_edge_exactly_once() {
    let (mut m, id) = logging_manager("trans_edges");
    // Two steady active ticks: Awake/OnEnable/Start once, then only Update.
    for _ in 0..2 {
        m.init_scripts();
        m.update_scripts(DT);
    }
    // Disable: OnDisable fires at the next tick's head; no Update while inactive.
    m.scene.borrow_mut().get_entity_mut(id).unwrap().active = false;
    for _ in 0..2 {
        m.init_scripts();
        m.update_scripts(DT);
    }
    // Re-enable: OnEnable fires (NOT Awake/Start again), then Update resumes.
    m.scene.borrow_mut().get_entity_mut(id).unwrap().active = true;
    m.init_scripts();
    m.update_scripts(DT);
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("A{id}E{id}S{id}U{id}U{id}D{id}E{id}U{id}"),
        "one OnDisable on the falling edge, one OnEnable on the rising edge, no repeats"
    );
}

#[test]
fn destroy_active_fires_disable_then_destroy_and_removes_entity() {
    let (mut m, id) = logging_manager("trans_destroy");
    m.init_scripts(); // Awake/OnEnable/Start
    m.eval("__log = ''").unwrap(); // isolate the teardown callbacks
    m.scene.borrow_mut().request_destroy(id);
    m.apply_pending_destroys();
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("D{id}X{id}"),
        "an active entity's destroy fires OnDisable then OnDestroy"
    );
    assert!(
        m.scene.borrow().get_entity(id).is_none(),
        "the entity is actually removed after the teardown dispatch"
    );
}

#[test]
fn destroy_already_disabled_fires_only_destroy() {
    let (mut m, id) = logging_manager("trans_destroy_disabled");
    m.init_scripts(); // Awake/OnEnable/Start
    m.scene.borrow_mut().get_entity_mut(id).unwrap().active = false;
    m.init_scripts(); // OnDisable (the falling edge)
    m.eval("__log = ''").unwrap(); // isolate the teardown callbacks
    m.scene.borrow_mut().request_destroy(id);
    m.apply_pending_destroys();
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("X{id}"),
        "destroying an already-disabled entity fires only OnDestroy (no second OnDisable)"
    );
}

#[test]
fn disabled_entity_receives_no_on_trigger() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(
        "trans_trigger",
        "_G.__t = false\nreturn { OnTrigger = function(id, other) _G.__t = true end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts(); // awoken while active
    m.scene.borrow_mut().get_entity_mut(id).unwrap().active = false;
    m.dispatch_trigger_events(TriggerEvents {
        stayed: vec![(id, 99)],
        ..Default::default()
    });
    assert_eq!(
        m.eval("__t").unwrap(),
        "false",
        "a disabled entity gets no OnTrigger — the same active gate as Update"
    );
}

/// Loads an `OnEnable`-tagged script at three slots (two entities, one carrying
/// two scripts), inits once, and returns the concatenated tags — the observable
/// enable dispatch order. A pure function of its inputs, so replays match.
fn on_enable_order_log() -> String {
    let (mut m, a) = manager_with_entity();
    let b = m.scene.borrow_mut().add_entity("E2".to_string());
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let tag = |name: &str, t: &str| {
        write_script(
            name,
            &format!("_G.__log = _G.__log or ''\nreturn {{ OnEnable = function(id) __log = __log .. '{t}' end }}"),
        )
    };
    m.load_entity_script(a, 0, &tag("tr_p", "P"), &BTreeMap::new())
        .unwrap();
    m.load_entity_script(a, 1, &tag("tr_q", "Q"), &BTreeMap::new())
        .unwrap();
    m.load_entity_script(b, 0, &tag("tr_r", "R"), &BTreeMap::new())
        .unwrap();
    m.init_scripts();
    m.eval("__log").unwrap()
}

#[test]
fn on_enable_dispatches_in_ascending_key_order_and_replays_identically() {
    let first = on_enable_order_log();
    assert_eq!(first, "PQR", "ascending (entity, script index) order");
    assert_eq!(
        first,
        on_enable_order_log(),
        "dispatch must be deterministic"
    );
}
