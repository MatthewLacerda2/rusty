//! Each entity's live script tables, as Unity's `GetComponent`: an owning script reaches a
//! widget's instance to read its state or hand it a callback.
//!
//! The index is a table in the Lua registry, `index[id][slot + 1] = { name, table }`,
//! written when a script loads and cleared when its entity is forgotten; a fresh
//! VM (every Play) starts with none. `Scene.GetScript` reads it.

use mlua::{Lua, Table, Value};

const KEY: &str = "rusty.script_instances";

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

pub(super) fn forget(lua: &Lua, id: u32) -> mlua::Result<()> {
    index(lua)?.set(id, Value::Nil)
}

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

pub fn handlers(lua: &Lua, id: u32, callback: &str) -> mlua::Result<Vec<Table>> {
    let Value::Table(slots) = index(lua)?.get::<Value>(id)? else {
        return Ok(Vec::new());
    };
    let mut hits: Vec<(i64, Table)> = Vec::new();
    for pair in slots.pairs::<i64, Table>() {
        let (slot, entry) = pair?;
        let table: Table = entry.get("table")?;
        if matches!(table.get::<Value>(callback)?, Value::Function(_)) {
            hits.push((slot, table));
        }
    }
    hits.sort_by_key(|(slot, _)| *slot);
    Ok(hits.into_iter().map(|(_, t)| t).collect())
}

impl super::ScriptManager {
    pub fn with_vm<R>(&self, f: impl FnOnce(&Lua) -> mlua::Result<R>) -> mlua::Result<Option<R>> {
        self.lua.as_ref().map(f).transpose()
    }

    /// For a caller in another VM, which cannot hold the table itself: a Lua table never
    /// leaves the VM that made it.
    pub fn with_script<R>(
        &self,
        id: u32,
        name: &str,
        f: impl FnOnce(&Lua, &Table) -> mlua::Result<R>,
    ) -> mlua::Result<Option<R>> {
        let Some(lua) = &self.lua else {
            return Ok(None);
        };
        find(lua, id, name)?.map(|t| f(lua, &t)).transpose()
    }
}
