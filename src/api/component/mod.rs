//! src/api/component/mod.rs — `Component` namespace (#1369): every serialized field
//! of a first-class component, read and written by name (Unity's public fields).
//!
//! - `Get(id, name)` — the entity's component as a table of its serialized fields,
//!   or `nil` without one.
//! - `Set(id, name, fields)` — write the named fields, all or nothing. Each goes
//!   through the `scene::authoring` op the inspector's card calls, so clamping,
//!   sanitizing and what follows a write are the inspector's own; a field with no
//!   op is read-only and writing it is an error naming why (`table`).
//! - `Fields(name)` — what an inspector lists: each field's name, the Lua type of
//!   its default, the default, and whether it can be written.
//!
//! Component names are the ones `Scene.AddComponent` takes, plus `Transform` and
//! `Mesh`; field names are as a saved scene spells them; enums travel by their
//! serialized name (read in any case, through `lua_json::decode`). An unknown
//! component or field is an error naming the valid ones. `#[serde(skip)]` runtime
//! state is never in a table: there is no field to name it by.

mod introspect;
mod kinds;
mod table;
mod value;
mod write;

#[cfg(test)]
mod lua_tests;
#[cfg(test)]
mod set_tests;
#[cfg(test)]
mod tests;

use std::cell::RefCell;

use mlua::{Lua, Table};
use serde_json::Value;

use super::{put, Reg};
use crate::scene::authoring;
use crate::scene::Scene;
use table::Kind;
use value::{from_lua, to_lua};

fn err(msg: String) -> mlua::Error {
    mlua::Error::RuntimeError(msg)
}

/// Register the `Component` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    let get = scope.create_function(move |lua, (id, name): (u32, String)| {
        let kind = kinds::find(&name).map_err(err)?;
        let value = (kind.read)(&scene.borrow().world, id)
            .transpose()
            .map_err(err)?;
        value.map(|v| to_lua(lua, &v)).transpose()
    });
    put(&t, "Get", get)?;
    let set = scope.create_function(move |lua, (id, name, fields): (u32, String, Table)| {
        let kind = kinds::find(&name).map_err(err)?;
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
        let written = (kind.write)(&mut scene.borrow_mut().world, id, &changes)
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
    let fields = scope.create_function(|lua, name: String| {
        let kind = kinds::find(&name).map_err(err)?;
        fields_table(lua, kind)
    });
    put(&t, "Fields", fields)?;
    lua.globals().set("Component", t).map_err(|e| e.to_string())
}

/// `Component.Fields`: one `{ name, type, default, writable, read_only }` row per
/// field, in the order a saved scene writes them. The defaults are the Add
/// Component menu's (a fresh entity's, for `Transform`; `Mesh` has none).
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

/// The component as the Add Component menu attaches it, serialized.
fn defaults(kind: &Kind) -> Result<Option<Value>, String> {
    let mut scene = Scene::new();
    let id = scene.add_entity("Defaults".to_string());
    if let Some(add) = kind.add {
        authoring::add_component(&mut scene, id, add);
    }
    (kind.read)(&scene.world, id).transpose()
}

/// A field's type as `Fields` reports it: the Lua type of its default (an enum
/// carrying data is a table), else of what its derive reads; `optional <type>`
/// when it can hold nothing.
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
