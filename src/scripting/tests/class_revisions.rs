//! The script-class cache across revisions and instances (#1710): a rewrite
//! reaches the next instance, a live one keeps its own, and each instance runs in
//! a closure of its own.

use std::collections::BTreeMap;
use std::path::Path;

use mlua::{Function, Table};

use super::classes::{field, spawn};
use super::lifecycle::{manager_with_entity, write_script};
use crate::core::fs::written::write;

/// The shared cache contract (#1207): a script missing at first loads once it is
/// written, and a rewrite at the same path loads its new contents.
#[test]
fn the_class_cache_follows_rewrites_at_one_path() {
    use rusty_test_support::rebakes::{follows_rebakes, Bake};
    let (mut m, _) = manager_with_entity();
    m.init_runtime().unwrap();
    let v = |which| if which == Bake::A { 1 } else { 2 };
    let path = crate::test_temp::dir().join("class_rebaked.lua");
    follows_rebakes(
        &path.to_string_lossy(),
        |p, which| {
            write(
                Path::new(p),
                format!("return {{ v = {} }}", v(which)).as_bytes(),
            )
            .unwrap()
        },
        |p| {
            let id = m.cells.scene.borrow_mut().add_entity("S".to_string());
            let loaded = m.load_entity_script(id, 0, p, &BTreeMap::new());
            loaded.ok().map(|()| field(&m, id, "v"))
        },
        |got, which| got.as_deref() == Some(v(which).to_string().as_str()),
    );
}

#[test]
fn live_instances_keep_the_revision_they_were_made_from() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("class_live", "return { v = 1 }");
    let old = spawn(&mut m, &path, &BTreeMap::new());
    std::fs::write(&path, "return { v = 2 }").unwrap();
    // Not in the write log yet: the class stands.
    let same = spawn(&mut m, &path, &BTreeMap::new());
    assert_eq!(field(&m, same, "v"), "1");
    write(Path::new(&path), b"return { v = 2 }").unwrap();
    let new = spawn(&mut m, &path, &BTreeMap::new());
    assert_eq!(
        (field(&m, old, "v"), field(&m, new, "v")),
        ("1".into(), "2".into())
    );
    assert_eq!(m.classes.reads, 2);
}

/// A script that gives itself private globals, as on `main` when every instance
/// compiled the file: one chunk called twice would share its `_ENV` upvalue.
const PRIVATE_GLOBALS: &str = "_ENV = setmetatable({}, { __index = _G })\n\
    return { set = function(v) mine = v end, get = function() return mine end }";

#[test]
fn a_top_level_env_stays_private_to_its_instance() {
    let (mut m, _) = manager_with_entity();
    m.init_runtime().unwrap();
    let path = write_script("class_private_env", PRIVATE_GLOBALS);
    let a = spawn(&mut m, &path, &BTreeMap::new());
    let b = spawn(&mut m, &path, &BTreeMap::new());
    let lua = m.lua.as_ref().unwrap();
    let table = |id| -> Table {
        lua.registry_value(&m.entity_scripts[&(id, 0)].table)
            .unwrap()
    };
    let call = |id, name: &str, arg: mlua::Value| -> mlua::Value {
        let f: Function = table(id).get(name).unwrap();
        f.call(arg).unwrap()
    };
    call(a, "set", mlua::Value::Integer(1));
    call(b, "set", mlua::Value::Integer(2));
    assert_eq!(call(a, "get", mlua::Value::Nil), mlua::Value::Integer(1));
    assert_eq!(call(b, "get", mlua::Value::Nil), mlua::Value::Integer(2));
    assert_eq!(
        m.eval("mine").unwrap(),
        "nil",
        "the VM's globals are untouched"
    );
    assert_eq!(m.classes.reads, 1);
}
