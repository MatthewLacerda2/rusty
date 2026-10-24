//! Tests for the transition callbacks (#323): `OnEnable`/`OnDisable` on the
//! entity's `active` edges, `OnDestroy` (preceded by `OnDisable` when active) on
//! the deferred destroy path, and the once-per-edge diff. Scope, the active gate
//! and dispatch order are in `active_gate.rs`.
//! Each callback tags a shared string: `A`wake, `E`nable, `S`tart, `U`pdate,
//! `D`isable, destro`X` — so one log asserts the exact order.

use std::collections::BTreeMap;

use super::lifecycle::{manager_with_entity, write_script};

pub(super) const DT: f32 = 1.0 / 60.0;

pub(super) const LOGGER: &str = "_G.__log = _G.__log or ''\n\
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
pub(super) fn logging_manager(name: &str) -> (crate::scripting::manager::ScriptManager, u32) {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
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
    m.cells.scene.borrow_mut().world.set_active(id, false);
    for _ in 0..2 {
        m.init_scripts();
        m.update_scripts(DT);
    }
    // Re-enable: OnEnable fires (NOT Awake/Start again), then Update resumes.
    m.cells.scene.borrow_mut().world.set_active(id, true);
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
    m.cells.scene.borrow_mut().request_destroy(id);
    m.apply_pending_destroys();
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("D{id}X{id}"),
        "an active entity's destroy fires OnDisable then OnDestroy"
    );
    assert!(
        !m.cells.scene.borrow().world.contains(id),
        "the entity is actually removed after the teardown dispatch"
    );
}

#[test]
fn destroy_already_disabled_fires_only_destroy() {
    let (mut m, id) = logging_manager("trans_destroy_disabled");
    m.init_scripts(); // Awake/OnEnable/Start
    m.cells.scene.borrow_mut().world.set_active(id, false);
    m.init_scripts(); // OnDisable (the falling edge)
    m.eval("__log = ''").unwrap(); // isolate the teardown callbacks
    m.cells.scene.borrow_mut().request_destroy(id);
    m.apply_pending_destroys();
    assert_eq!(
        m.eval("__log").unwrap(),
        format!("X{id}"),
        "destroying an already-disabled entity fires only OnDestroy (no second OnDisable)"
    );
}
