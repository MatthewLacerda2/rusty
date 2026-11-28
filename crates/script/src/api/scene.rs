use std::collections::BTreeMap;

use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::ScriptFieldValue;
use rusty_edit::authoring::{self, ComponentKind, Primitive};
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>, scene_path: ApiCell<Option<String>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_lookup_create(lua, &table, scene)?;
    register_destroy(lua, &table, scene)?;
    register_components(lua, &table, scene)?;
    register_parenting(lua, &table, scene)?;
    crate::api::scene_prefab::register(lua, &table, scene)?;
    crate::api::scene_load::register(lua, &table, scene, scene_path)?;
    crate::api::scene_hierarchy::register(lua, &table, scene)?;

    lua.globals().set("Scene", table).map_err(|e| e.to_string())
}

fn register_lookup_create(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "FindEntityByName",
        lua.create_function(move |lua, name: String| {
            Ok(scene.get(lua)?.borrow().find_entity_by_name(&name))
        }),
    )?;

    put(
        table,
        "CreateEntity",
        lua.create_function(move |lua, (name, primitive): (String, Option<String>)| {
            let primitive = primitive.as_deref().and_then(Primitive::parse);
            Ok(authoring::create_entity(
                &mut scene.get(lua)?.borrow_mut(),
                &name,
                primitive,
            ))
        }),
    )
}

/// `Deactivate` (Unity's deferred destroy) + `DestroyEntity` (real removal).
fn register_destroy(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Deactivate",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.live("Scene", "Deactivate")?;
            scene.get(lua)?.borrow_mut().world.set_active(id, false);
            Ok(())
        }),
    )?;

    put(
        table,
        "DestroyEntity",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Scene", "DestroyEntity")?;
            let mut s = scene.borrow_mut();
            let existed = s.world.contains(id);
            if existed {
                s.destroy(id);
            }
            Ok(existed)
        }),
    )
}

fn register_components(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "AddComponent",
        lua.create_function(move |lua, (id, kind): (EntityArg, String)| {
            let id = id.live("Scene", "AddComponent")?;
            let parsed = ComponentKind::parse_or_list(&kind).map_err(mlua::Error::RuntimeError)?;
            Ok(authoring::add_component(
                &mut scene.get(lua)?.borrow_mut(),
                id,
                parsed,
            ))
        }),
    )?;

    put(
        table,
        "RemoveComponent",
        lua.create_function(move |lua, (id, kind): (EntityArg, String)| {
            let id = id.live("Scene", "RemoveComponent")?;
            let parsed = ComponentKind::parse_or_list(&kind).map_err(mlua::Error::RuntimeError)?;
            Ok(authoring::remove_component(
                &mut scene.get(lua)?.borrow_mut(),
                id,
                parsed,
            ))
        }),
    )?;

    put(
        table,
        "AddScript",
        lua.create_function(
            move |lua, (id, path, fields): (EntityArg, String, Option<mlua::Table>)| {
                let id = id.live("Scene", "AddScript")?;
                let values = field_values(fields)?;
                Ok(authoring::add_script(
                    &mut scene.get(lua)?.borrow_mut().world,
                    id,
                    &path,
                    values,
                ))
            },
        ),
    )
}

fn field_values(fields: Option<mlua::Table>) -> mlua::Result<BTreeMap<String, ScriptFieldValue>> {
    let Some(fields) = fields else {
        return Ok(BTreeMap::new());
    };
    fields
        .pairs::<String, mlua::Value>()
        .map(|pair| {
            let (name, value) = pair?;
            let value = match value {
                mlua::Value::Integer(n) => ScriptFieldValue::Number(n as f64),
                mlua::Value::Number(n) => ScriptFieldValue::Number(n),
                mlua::Value::Boolean(b) => ScriptFieldValue::Boolean(b),
                mlua::Value::String(s) => ScriptFieldValue::Text(s.to_str()?.to_string()),
                other => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Scene.AddScript: field '{name}' is a {}; fields are numbers, booleans or strings",
                        other.type_name()
                    )))
                }
            };
            Ok((name, value))
        })
        .collect()
}

fn register_parenting(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetParent",
        lua.create_function(move |lua, (id, parent): (EntityArg, EntityArg)| {
            let id = id.live("Scene", "SetParent")?;
            // `nil` unparents, Unity's `SetParent(null)`.
            scene
                .get(lua)?
                .borrow_mut()
                .reparent(id, parent.get())
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "ClearParent",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.live("Scene", "ClearParent")?;
            scene
                .get(lua)?
                .borrow_mut()
                .reparent(id, None)
                .map_err(mlua::Error::RuntimeError)
        }),
    )
}
