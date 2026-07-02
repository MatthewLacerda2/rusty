//! Tests for the trigger-edge callbacks (#310): `OnTriggerEnter` / `OnTrigger`
//! (stay) / `OnTriggerExit` dispatch in that fixed order, each fed by its own
//! event list.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use super::tests_lifecycle::{manager_with_entity, write_script};
use crate::physics::TriggerEvents;

#[test]
fn dispatch_fires_enter_stay_exit_in_order() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(
        "edges",
        "_G.__log = ''\nreturn {\n\
         OnTriggerEnter = function(id, other) __log = __log .. 'E' .. other end,\n\
         OnTrigger = function(id, other) __log = __log .. 'S' end,\n\
         OnTriggerExit = function(id, other) __log = __log .. 'X' .. other end,\n\
         }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();

    // Overlap begins: enter fires before that tick's stay.
    m.dispatch_trigger_events(TriggerEvents {
        entered: vec![(id, 9)],
        stayed: vec![(id, 9)],
        ..Default::default()
    });
    assert_eq!(m.eval("__log").unwrap(), "E9S");

    // Overlap persists: stay only, no repeated enter.
    m.dispatch_trigger_events(TriggerEvents {
        stayed: vec![(id, 9)],
        ..Default::default()
    });
    assert_eq!(m.eval("__log").unwrap(), "E9SS");

    // Overlap ends: exit only, carrying the other entity's id.
    m.dispatch_trigger_events(TriggerEvents {
        exited: vec![(id, 9)],
        ..Default::default()
    });
    assert_eq!(m.eval("__log").unwrap(), "E9SSX9");
}

#[test]
fn missing_edge_callbacks_are_optional() {
    // A script defining only the legacy OnTrigger keeps working untouched.
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(
        "stay_only",
        "_G.__n = 0\nreturn { OnTrigger = function() __n = __n + 1 end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.dispatch_trigger_events(TriggerEvents {
        entered: vec![(id, 7)],
        stayed: vec![(id, 7)],
        ..Default::default()
    });
    m.dispatch_trigger_events(TriggerEvents {
        exited: vec![(id, 7)],
        ..Default::default()
    });
    assert_eq!(m.eval("__n").unwrap(), "1", "only the stay tick counted");
}
