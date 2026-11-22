use std::collections::BTreeMap;

use super::lifecycle::{manager_with_entity, write_script};
use rusty_physics::TriggerEvents;

#[test]
fn dispatch_fires_enter_stay_exit_in_order() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
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
    m.init_scripts();

    m.dispatch_trigger_events(TriggerEvents {
        entered: vec![(id, 9)],
        stayed: vec![(id, 9)],
        ..Default::default()
    });
    assert_eq!(m.eval("__log").unwrap(), "E9S");

    m.dispatch_trigger_events(TriggerEvents {
        stayed: vec![(id, 9)],
        ..Default::default()
    });
    assert_eq!(m.eval("__log").unwrap(), "E9SS");

    m.dispatch_trigger_events(TriggerEvents {
        exited: vec![(id, 9)],
        ..Default::default()
    });
    assert_eq!(m.eval("__log").unwrap(), "E9SSX9");
}

#[test]
fn missing_edge_callbacks_are_optional() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script(
        "stay_only",
        "_G.__n = 0\nreturn { OnTrigger = function() __n = __n + 1 end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts();
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
