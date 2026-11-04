//! The script-class cache (#1710): a script is read and compiled once per VM, and
//! every instance is a fresh table from the cached chunk, with its own fields.

use std::collections::BTreeMap;

use mlua::Table;

use super::lifecycle::{manager_with_entity, write_script};
use crate::components::ScriptFieldValue;
use crate::scripting::manager::ScriptManager;

const COUNTED: &str = "__made = (__made or 0) + 1\n\
                       return { fields = { speed = 3, label = 'x' }, n = __made }";

pub(super) fn field(m: &ScriptManager, id: u32, key: &str) -> String {
    let lua = m.lua.as_ref().unwrap();
    let table: Table = lua
        .registry_value(&m.entity_scripts[&(id, 0)].table)
        .unwrap();
    table.get::<mlua::Value>(key).unwrap().to_string().unwrap()
}

pub(super) fn spawn(
    m: &mut ScriptManager,
    path: &str,
    values: &BTreeMap<String, ScriptFieldValue>,
) -> u32 {
    let id = m.cells.scene.borrow_mut().add_entity("S".to_string());
    m.load_entity_script(id, 0, path, values).unwrap();
    id
}

#[test]
fn a_thousand_instances_of_one_script_read_it_once() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("class_thousand", COUNTED);
    let fast = BTreeMap::from([("speed".to_string(), ScriptFieldValue::Number(9.0))]);
    let none = BTreeMap::new();
    let ids: Vec<u32> = (0..1000)
        .map(|i| spawn(&mut m, &path, if i % 2 == 0 { &fast } else { &none }))
        .collect();
    assert_eq!(m.classes.reads, 1);
    // Each instance is its own table: the top level ran once per instance, and the
    // fields are the defaults under that instance's own overrides.
    assert_eq!(m.eval("__made").unwrap(), "1000");
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(field(&m, id, "n"), (i + 1).to_string());
        assert_eq!(field(&m, id, "speed"), if i % 2 == 0 { "9" } else { "3" });
        assert_eq!(field(&m, id, "label"), "x");
    }
}

#[test]
fn a_script_that_does_not_compile_is_read_once_and_fails_every_instance() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("class_broken", "return {");
    for _ in 0..3 {
        let id = m.cells.scene.borrow_mut().add_entity("S".to_string());
        let err = m.load_entity_script(id, 0, &path, &BTreeMap::new());
        let err = err.unwrap_err();
        assert!(err.starts_with("Syntax error compiling"), "{err}");
        assert!(err.contains(&format!("{path}:1:")), "named after its file");
    }
    assert_eq!(m.classes.reads, 1);
}

#[test]
fn a_top_level_runtime_error_is_not_called_a_syntax_error() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("class_throws", "error('boom')\nreturn {}");
    let err = m.load_entity_script(id, 0, &path, &BTreeMap::new());
    let err = err.unwrap_err();
    assert!(err.starts_with(&format!("Error running {path}")), "{err}");
    assert!(err.contains(&format!("{path}:1: boom")), "{err}");
}

#[test]
fn a_missing_script_is_looked_for_again() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = crate::test_temp::dir().join("class_late.lua");
    let path = path.to_string_lossy();
    assert!(m
        .load_entity_script(id, 0, &path, &BTreeMap::new())
        .is_err());
    std::fs::write(&*path, "return { v = 7 }").unwrap();
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    assert_eq!(field(&m, id, "v"), "7");
}

#[test]
fn a_bare_table_script_loads_as_an_expression() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("class_bare", "{ v = 4 }");
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    assert_eq!(field(&m, id, "v"), "4");
}

#[test]
fn a_fresh_vm_reads_every_script_again() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("class_fresh_vm", "return { v = 1 }");
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    std::fs::write(&path, "return { v = 5 }").unwrap();
    m.init_runtime().unwrap();
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    assert_eq!(field(&m, id, "v"), "5");
}
