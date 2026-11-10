//! src/api/scene.rs — `Scene` namespace.
//!
//! The structural-authoring surface, mirroring the editor's GameObject menu and
//! the inspector's Add Component menu (issue #181). Every verb routes through the
//! shared `scene::authoring` module so the API and the editor can never drift:
//!
//!   - `FindEntityByName(name)` — lookup, `nil` if absent (Unity's `GameObject.Find` returns `null`).
//!   - `CreateEntity(name, [primitive])` — the GameObject menu's primitive set.
//!   - `Deactivate(id)` — Unity's deferred `Object.Destroy` (sets `active = false`).
//!   - `DestroyEntity(id)` — **really** removes the entity (the editor's Destroy).
//!   - `AddComponent(id, kind)` / `RemoveComponent(id, kind)` — the Add Component
//!     menu's set, with the inspector's default values.
//!   - `AddScript(id, path, [fields])` — the Add Component menu's script entries.
//!   - `SetParent(id, parent)` / `ClearParent(id)` — parenting (`parent_id`).
//!   - `Save([path])` — persist the live world (write-back to the current scene
//!     path when no path is given).
//!   - `New` / `Load` / `GetActivePath` / `DontDestroyOnLoad` — scene loading (#432), in
//!     `api::scene_load`.
//!
//! These verbs deliberately do not touch the `ConsoleLogs` cell: when the REPL /
//! headless session drives them, `console::evaluate_line` already holds that cell
//! mutably (and logs the returned value), so a second borrow here would panic.
//! Each verb returns its outcome instead, which the REPL echoes.

use std::collections::BTreeMap;

use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::ScriptFieldValue;
use crate::scene::authoring::{self, ComponentKind, Primitive};
use crate::scene::Scene;

/// Register the `Scene` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>, scene_path: ApiCell<Option<String>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_lookup_create(lua, &table, scene)?;
    register_destroy(lua, &table, scene)?;
    register_components(lua, &table, scene)?;
    register_parenting(lua, &table, scene)?;
    register_save(lua, &table, scene, scene_path)?;
    crate::api::scene_prefab::register(lua, &table, scene)?;
    crate::api::scene_load::register(lua, &table, scene, scene_path)?;
    crate::api::scene_hierarchy::register(lua, &table, scene)?;

    lua.globals().set("Scene", table).map_err(|e| e.to_string())
}

/// `FindEntityByName` + `CreateEntity`.
fn register_lookup_create(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "FindEntityByName",
        lua.create_function(move |lua, name: String| {
            Ok(scene.get(lua)?.borrow().find_entity_by_name(&name))
        }),
    )?;

    // `CreateEntity(name, [primitive])`: an absent/empty/unknown primitive name
    // creates a bare transform-only entity, matching the GameObject menu's Create
    // Empty. Returns the new entity's stable id.
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
    // Soft delete — mirrors Unity's deferred `Object.Destroy`. The entity stays in
    // the scene but inactive; this is the historical `DestroyEntity` behaviour,
    // renamed so the destructive verb can own the `DestroyEntity` name.
    put(
        table,
        "Deactivate",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.live("Scene", "Deactivate")?;
            scene.get(lua)?.borrow_mut().world.set_active(id, false);
            Ok(())
        }),
    )?;

    // Hard delete — the editor's Edit ▸ Delete: actually removes the entity.
    // Returns whether an entity actually existed at `id`.
    //
    // During PLAY the removal is DEFERRED (#323): the id is queued and the entity
    // stays live until the script system's destroy phase, which fires its
    // `OnDisable`/`OnDestroy` before removing it — matching Unity's end-of-frame
    // `Object.Destroy`. In EDIT mode there is no script tick to drain the queue
    // (and no scripts to notify), so it is removed at once (and the selection
    // cleared if it was selected). `Scene::destroy` picks, from the scene's one
    // play state (#1302).
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

/// `AddComponent` / `RemoveComponent` over the Add Component menu's set, and
/// `AddScript` for its script entries.
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

/// `AddScript`'s `fields` table as the inspector's stored field values.
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

/// `SetParent` / `ClearParent` (the `parent_id` graph, cycle-checked by `Scene`).
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

/// `Save([path])` — persist the live world. No path writes back to the current
/// scene file (the shared `scene_path` cell); an explicit path writes there and
/// also becomes the new current scene file (mirroring the editor's Save As).
/// Refused during Play by the scene layer (#1340).
fn register_save(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    scene_path: ApiCell<Option<String>>,
) -> Reg {
    put(
        table,
        "Save",
        lua.create_function(move |lua, path: Option<String>| {
            let scene = &*scene.get(lua)?;
            let scene_path = &*scene_path.get(lua)?;
            let target = match path.or_else(|| scene_path.borrow().clone()) {
                Some(p) => p,
                None => {
                    return Err(mlua::Error::RuntimeError(
                        "Scene.Save: no path given and no current scene file is set".to_string(),
                    ))
                }
            };
            scene
                .borrow()
                .save_to_file(&target)
                .map_err(|e| mlua::Error::RuntimeError(format!("Scene.Save: {e}")))?;
            *scene_path.borrow_mut() = Some(target.clone());
            Ok(target)
        }),
    )
}
