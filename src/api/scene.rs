//! src/api/scene.rs — `Scene` namespace.
//!
//! The structural-authoring surface, mirroring the editor's GameObject menu and
//! the inspector's Add Component menu (issue #181). Every verb routes through the
//! shared `scene::authoring` module so the API and the editor can never drift:
//!
//!   - `FindEntityByName(name)` — lookup, `0` if absent.
//!   - `CreateEntity(name, [primitive])` — the GameObject menu's primitive set.
//!   - `Deactivate(id)` — Unity's deferred `Object.Destroy` (sets `active = false`).
//!   - `DestroyEntity(id)` — **really** removes the entity (the editor's Destroy).
//!   - `AddComponent(id, kind)` / `RemoveComponent(id, kind)` — the Add Component
//!     menu's set, with the inspector's default values.
//!   - `SetParent(id, parent)` / `ClearParent(id)` — parenting (`parent_id`).
//!   - `Save([path])` — persist the live world (write-back to the current scene
//!     path when no path is given).
//!
//! These verbs deliberately do not touch the `ConsoleLogs` cell: when the REPL /
//! headless session drives them, `console::evaluate_line` already holds that cell
//! mutably (and logs the returned value), so a second borrow here would panic.
//! Each verb returns its outcome instead, which the REPL echoes.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::authoring::{self, ComponentKind, Primitive};
use crate::scene::Scene;

/// Register the `Scene` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
    is_playing: &'scope RefCell<bool>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_lookup_create(scope, &table, scene)?;
    register_destroy(scope, &table, scene, is_playing)?;
    register_components(scope, &table, scene)?;
    register_parenting(scope, &table, scene)?;
    register_save(scope, &table, scene, scene_path)?;
    crate::api::scene_prefab::register(scope, &table, scene)?;

    lua.globals().set("Scene", table).map_err(|e| e.to_string())
}

/// `FindEntityByName` + `CreateEntity`.
fn register_lookup_create<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "FindEntityByName",
        scope.create_function(|_, name: String| Ok(scene.borrow().find_entity_by_name(&name))),
    )?;

    // `CreateEntity(name, [primitive])`: an absent/empty/unknown primitive name
    // creates a bare transform-only entity, matching the GameObject menu's Create
    // Empty. Returns the new entity's stable id.
    put(
        table,
        "CreateEntity",
        scope.create_function(|_, (name, primitive): (String, Option<String>)| {
            let primitive = primitive.as_deref().and_then(Primitive::parse);
            Ok(authoring::create_entity(
                &mut scene.borrow_mut(),
                &name,
                primitive,
            ))
        }),
    )
}

/// `Deactivate` (Unity's deferred destroy) + `DestroyEntity` (real removal).
fn register_destroy<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    is_playing: &'scope RefCell<bool>,
) -> Reg {
    // Soft delete — mirrors Unity's deferred `Object.Destroy`. The entity stays in
    // the scene but inactive; this is the historical `DestroyEntity` behaviour,
    // renamed so the destructive verb can own the `DestroyEntity` name.
    put(
        table,
        "Deactivate",
        scope.create_function(|_, id: u32| {
            let mut s = scene.borrow_mut();
            if let Some(mut e) = s.get_entity_mut(id) {
                e.active = false;
            }
            Ok(())
        }),
    )?;

    // Hard delete — the editor's Destroy button: actually removes the entity.
    // Returns whether an entity actually existed at `id`.
    //
    // During PLAY the removal is DEFERRED (#323): the id is queued and the entity
    // stays live until the script system's destroy phase, which fires its
    // `OnDisable`/`OnDestroy` before removing it — matching Unity's end-of-frame
    // `Object.Destroy`. In EDIT mode there is no script tick to drain the queue
    // (and no scripts to notify), so removal is immediate, via `Scene::destroy_entity`
    // (which also clears the selection if it was selected).
    put(
        table,
        "DestroyEntity",
        scope.create_function(|_, id: u32| {
            let mut s = scene.borrow_mut();
            let existed = s.get_entity(id).is_some();
            if *is_playing.borrow() {
                if existed {
                    s.request_destroy(id);
                }
            } else {
                s.destroy_entity(id);
            }
            Ok(existed)
        }),
    )
}

/// `AddComponent` / `RemoveComponent` over the Add Component menu's set.
fn register_components<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "AddComponent",
        scope.create_function(|_, (id, kind): (u32, String)| {
            let parsed = ComponentKind::parse(&kind).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("unknown component '{}'", kind))
            })?;
            Ok(authoring::add_component(
                &mut scene.borrow_mut(),
                id,
                parsed,
            ))
        }),
    )?;

    put(
        table,
        "RemoveComponent",
        scope.create_function(|_, (id, kind): (u32, String)| {
            let parsed = ComponentKind::parse(&kind).ok_or_else(|| {
                mlua::Error::RuntimeError(format!("unknown component '{}'", kind))
            })?;
            Ok(authoring::remove_component(
                &mut scene.borrow_mut(),
                id,
                parsed,
            ))
        }),
    )
}

/// `SetParent` / `ClearParent` (the `parent_id` graph, cycle-checked by `Scene`).
fn register_parenting<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetParent",
        scope.create_function(|_, (id, parent): (u32, u32)| {
            scene
                .borrow_mut()
                .set_parent(id, Some(parent))
                .map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "ClearParent",
        scope.create_function(|_, id: u32| {
            scene
                .borrow_mut()
                .set_parent(id, None)
                .map_err(mlua::Error::RuntimeError)
        }),
    )
}

/// `Save([path])` — persist the live world. No path writes back to the current
/// scene file (the shared `scene_path` cell); an explicit path writes there and
/// also becomes the new current scene file (mirroring the editor's Save As).
fn register_save<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    scene_path: &'scope RefCell<Option<String>>,
) -> Reg {
    put(
        table,
        "Save",
        scope.create_function(|_, path: Option<String>| {
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
                .map_err(mlua::Error::RuntimeError)?;
            *scene_path.borrow_mut() = Some(target.clone());
            Ok(target)
        }),
    )
}
