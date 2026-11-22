mod value;

#[cfg(test)]
mod lua_tests;
#[cfg(test)]
mod set_tests;
#[cfg(test)]
mod since_get_tests;

use mlua::{Lua, Table};
use serde_json::Value;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_edit::authoring;
use rusty_edit::authoring::fields::{self as kinds, Kind, Skip};
use rusty_scene::Scene;
use value::{from_lua, to_lua};

/// The metatable key holding what `Get` returned, so `Set` writes only the fields the script
/// edited, not those the sim moved since.
const GOT: &str = "__component_get";

fn err(msg: String) -> mlua::Error {
    mlua::Error::RuntimeError(msg)
}

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    let get = lua.create_function(move |lua, (id, name): (EntityArg, String)| {
        let id = id.read();
        let kind = kinds::find(&name).map_err(err)?;
        let value = (kind.read)(&scene.get(lua)?.borrow().world, id)
            .transpose()
            .map_err(err)?;
        let Some(value) = value else {
            return Ok(mlua::Value::Nil);
        };
        let table = to_lua(lua, &value)?;
        if let mlua::Value::Table(t) = &table {
            remember(lua, t, id, kind, &value)?;
        }
        Ok(table)
    });
    put(&t, "Get", get)?;
    let set = lua.create_function(move |lua, (id, name, fields): (EntityArg, String, Table)| {
        let id = id.live("Component", "Set")?;
        let kind = kinds::find(&name).map_err(err)?;
        let got = recall(&fields, id, kind);
        let skip = got.as_ref().map_or(Skip::Held, Skip::SinceGet);
        let fields = from_lua(lua, &mlua::Value::Table(fields))
            .map_err(|e| err(format!("Component.Set: {}: {e}", kind.name)))?;
        let changes = match fields {
            Value::Object(map) => map,
            _ => {
                return Err(err(format!(
                    "Component.Set: {} fields must be a table of name = value",
                    kind.name
                )))
            }
        };
        let written = (kind.write)(&mut scene.get(lua)?.borrow_mut().world, id, &changes, skip)
            .map_err(|e| err(format!("Component.Set: {}: {e}", kind.name)))?;
        if !written {
            return Err(err(format!(
                "Component.Set: entity {id} has no {} (Scene.AddComponent first)",
                kind.name
            )));
        }
        Ok(())
    });
    put(&t, "Set", set)?;
    let fields = lua.create_function(|lua, name: String| {
        let kind = kinds::find(&name).map_err(err)?;
        fields_table(lua, kind)
    });
    put(&t, "Fields", fields)?;
    lua.globals().set("Component", t).map_err(|e| e.to_string())
}

fn remember(lua: &Lua, table: &Table, id: u32, kind: &Kind, value: &Value) -> mlua::Result<()> {
    let got = lua.create_table()?;
    got.set("id", id)?;
    got.set("kind", kind.name)?;
    got.set("value", value.to_string())?;
    let meta = lua.create_table()?;
    meta.set(GOT, got)?;
    table.set_metatable(Some(meta))
}

fn recall(table: &Table, id: u32, kind: &Kind) -> Option<serde_json::Map<String, Value>> {
    let got: Table = table.metatable()?.raw_get(GOT).ok()?;
    let same = got.get::<u32>("id").ok()? == id && got.get::<String>("kind").ok()? == kind.name;
    match serde_json::from_str(&got.get::<String>("value").ok()?).ok()? {
        Value::Object(map) if same => Some(map),
        _ => None,
    }
}

fn fields_table(lua: &Lua, kind: &Kind) -> mlua::Result<Table> {
    let defaults = defaults(kind).map_err(err)?;
    let rows = lua.create_table()?;
    for (i, (name, read_only)) in (kind.fields)().into_iter().enumerate() {
        let default = defaults.as_ref().and_then(|d| d.get(name));
        let row = lua.create_table()?;
        row.set("name", name)?;
        row.set("type", type_name(kind, name, default))?;
        row.set("default", default.map(|v| to_lua(lua, v)).transpose()?)?;
        row.set("writable", read_only.is_none())?;
        row.set("read_only", read_only)?;
        rows.set(i + 1, row)?;
    }
    Ok(rows)
}

fn defaults(kind: &Kind) -> Result<Option<Value>, String> {
    let mut scene = Scene::new();
    let id = scene.add_entity("Defaults".to_string());
    if let Some(add) = kind.add {
        authoring::add_component(&mut scene, id, add);
    }
    (kind.read)(&scene.world, id).transpose()
}

fn type_name(kind: &Kind, name: &'static str, default: Option<&Value>) -> String {
    let (optional, read_as) = (kind.field_type)(name);
    let held = match default {
        None | Some(Value::Null) => None,
        Some(Value::Bool(_)) => Some("boolean"),
        Some(Value::Number(_)) => Some("number"),
        Some(Value::String(_)) => Some("string"),
        Some(Value::Array(_) | Value::Object(_)) => Some("table"),
    };
    let lua = held.or(read_as).unwrap_or("any");
    if optional {
        format!("optional {lua}")
    } else {
        lua.to_owned()
    }
}
