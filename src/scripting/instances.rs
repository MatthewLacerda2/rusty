//! src/scripting/instances.rs — the live script tables, by entity (#422).
//!
//! Unity's `GetComponent<Button>()`: an owning script reaches a widget's script
//! instance to read its state or hand it a callback (`button.on_click = fn`). The
//! index is a table in the Lua registry, `index[id][slot + 1] = { name, table }`,
//! written when a script loads and cleared when its entity is forgotten; a fresh
//! VM (every Play) starts with none. `Scene.GetScript` reads it.

use mlua::{Lua, Table, Value};

/// The registry name of the index.
const KEY: &str = "rusty.script_instances";

/// The index table, created on first use.
fn index(lua: &Lua) -> mlua::Result<Table> {
    match lua.named_registry_value::<Value>(KEY)? {
        Value::Table(t) => Ok(t),
        _ => {
            let t = lua.create_table()?;
            lua.set_named_registry_value(KEY, t.clone())?;
            Ok(t)
        }
    }
}

/// Record `table` as entity `id`'s script in `slot`, named `name` (the file stem).
pub(super) fn record(
    lua: &Lua,
    id: u32,
    slot: usize,
    name: &str,
    table: &Table,
) -> mlua::Result<()> {
    let index = index(lua)?;
    let slots = match index.get::<Value>(id)? {
        Value::Table(t) => t,
        _ => {
            let t = lua.create_table()?;
            index.set(id, t.clone())?;
            t
        }
    };
    let entry = lua.create_table()?;
    entry.set("name", name)?;
    entry.set("table", table.clone())?;
    slots.set(slot + 1, entry)
}

/// Drop every script of entity `id`.
pub(super) fn forget(lua: &Lua, id: u32) -> mlua::Result<()> {
    index(lua)?.set(id, Value::Nil)
}

/// Entity `id`'s first script (lowest slot) named `name`, or `nil`.
pub fn find(lua: &Lua, id: u32, name: &str) -> mlua::Result<Option<Table>> {
    let Value::Table(slots) = index(lua)?.get::<Value>(id)? else {
        return Ok(None);
    };
    let mut hits: Vec<(i64, Table)> = Vec::new();
    for pair in slots.pairs::<i64, Table>() {
        let (slot, entry) = pair?;
        if entry.get::<String>("name")? == name {
            hits.push((slot, entry.get("table")?));
        }
    }
    hits.sort_by_key(|(slot, _)| *slot);
    Ok(hits.into_iter().next().map(|(_, t)| t))
}
