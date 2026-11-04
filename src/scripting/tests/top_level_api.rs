//! A script that reads the API at its top level (#1709): the gameplay VM has the
//! surface from the start, and the inspection VM stubs it, so the script's
//! `fields` still load, it is still discovered, and a schema that cannot be read
//! warns once instead of silently dropping its values.

use std::collections::BTreeMap;

use super::lifecycle::{manager_with_entity, write_script};
use super::timers::tick;
use crate::components::ScriptFieldValue;
use crate::scripting::console::LogLevel;
use crate::scripting::discovery::exposes_lifecycle;
use crate::scripting::parse_fields;

const READS_API: &str = "local now = Time.time\nlocal step = Time.deltaTime() * 2\n\
                         local M = { fields = { speed = 3, label = 'x' } }\n\
                         function M.Start(id) __speed = M.speed end\nreturn M";

#[test]
fn a_chunk_reading_the_api_at_load_keeps_its_fields_and_lifecycle() {
    let names: Vec<_> = parse_fields(READS_API)
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(names, ["label", "speed"]);
    assert!(exposes_lifecycle(READS_API));
}

#[test]
fn the_loader_applies_fields_of_a_chunk_reading_the_api() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("top_level_api", READS_API);
    let values = BTreeMap::from([("speed".to_string(), ScriptFieldValue::Number(9.0))]);
    m.load_entity_script(id, 0, &path, &values).unwrap();
    tick(&mut m, 1);
    assert_eq!(m.eval("__speed").unwrap(), "9");
    let messages = &m.cells.console.borrow().messages;
    assert!(messages.is_empty(), "{messages:?}");
}

#[test]
fn an_unreadable_schema_warns_once_per_script() {
    // Errors in the inspection VM only: `__gameplay` is set before load there.
    let code = "if not rawget(_G, '__gameplay') then error('boom') end\n\
                return { fields = { speed = 3 }, Start = function(id) end }";
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    m.eval("__gameplay = true").unwrap();
    let path = write_script("top_level_unreadable", code);
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.load_entity_script(id, 1, &path, &BTreeMap::new())
        .unwrap();
    let messages = &m.cells.console.borrow().messages;
    let warned: Vec<_> = messages
        .iter()
        .filter(|(msg, level)| *level == LogLevel::Warning && msg.contains(&path))
        .collect();
    assert_eq!(warned.len(), 1, "{messages:?}");
}
