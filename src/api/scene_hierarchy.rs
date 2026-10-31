//! src/api/scene_hierarchy.rs — the `Scene` namespace's hierarchy reads (#422) and an
//! entity's own facts (name, active, static).
//!
//! Registered onto the SAME `Scene` table as `api::scene`. Unity's `transform.parent`,
//! `transform.GetChild` / `childCount`, `transform.Find` and `GetComponent<T>()` for
//! script components — what a widget script needs to reach its own parts (a
//! slider's handle, a toggle's checkmark) without storing ids that a prefab stamp
//! would invalidate — and the GameObject's `SetActive` and Static flags:
//!
//!   - `GetParent(id)` — the parent's id, or `nil`.
//!   - `GetChildren(id)` — the child ids in hierarchy order (an empty array for none).
//!   - `FindChild(id, path)` — a descendant by name path (`"Viewport/Content"`),
//!     one level per segment, the first match in hierarchy order; `nil` if absent.
//!   - `GetName(id)` / `SetName(id, name)` — Unity's read/write `gameObject.name`
//!     (#1611): the same `World::set_name` the inspector's name field writes, so a
//!     rename is saved with the scene. Empty and duplicate names are allowed, as in
//!     Unity.
//!   - `SetActive(id, bool)` / `IsActive(id)` — Unity's `SetActive` / `activeSelf`:
//!     the entity's own flag (an inactive ancestor still hides it).
//!     `IsActiveInHierarchy(id)` — Unity's `activeInHierarchy`: it and every
//!     ancestor are active, the rule every system follows (#1363). A pooled
//!     object comes back with `SetActive(id, true)`; `Deactivate` is the old
//!     one-way soft delete.
//!   - `SetStatic(id, bool)` / `IsStatic(id)` — the inspector's Static checkbox
//!     (`World::set_static`, #1496). Live, unlike Unity's editor-only `isStatic`:
//!     physics, the static shadow bake and probe lighting follow it next frame; the
//!     navmesh and lightmap at their next bake.
//!   - `GetScript(id, name)` — entity `id`'s live script instance named `name` (the
//!     file stem, `"button"`), the very table its callbacks run on; `nil` outside
//!     play or when it has none.
//!
//! A `nil` id (a failed `FindEntityByName`) reads as a missing entity, as every
//! read's does (`api::entity_arg`): `GetName`, `GetParent`, `FindChild` and `GetScript`
//! answer `nil`, `GetChildren` an empty array and `IsActive` / `IsStatic` `false`.

use std::cell::RefCell;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::scene::Scene;
use crate::scripting::instances;

/// Register the hierarchy reads onto the (already-created) `Scene` `table`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(scene.borrow().world.name(id.read()).map(|n| n.clone()))
    });
    put(table, "GetName", f)?;
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let id = id.live("Scene", "SetName")?;
        Ok(scene.borrow_mut().world.set_name(id, name))
    });
    put(table, "SetName", f)?;
    let f = scope
        .create_function(move |_, id: EntityArg| Ok(scene.borrow().world.parent_id(id.read())));
    put(table, "GetParent", f)?;
    let f =
        scope.create_function(move |_, id: EntityArg| Ok(scene.borrow().world.children(id.read())));
    put(table, "GetChildren", f)?;
    let f = scope.create_function(move |_, (id, path): (EntityArg, String)| {
        Ok(scene.borrow().world.find_child(id.read(), &path))
    });
    put(table, "FindChild", f)?;
    let f = scope.create_function(move |_, (id, on): (EntityArg, bool)| {
        let id = id.live("Scene", "SetActive")?;
        Ok(scene.borrow_mut().world.set_active(id, on))
    });
    put(table, "SetActive", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        // lint: allow(active-self) `IsActive` is Unity's `activeSelf`
        Ok(scene.borrow().world.is_active_self(id.read()))
    });
    put(table, "IsActive", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(scene.borrow().world.is_active_in_hierarchy(id.read()))
    });
    put(table, "IsActiveInHierarchy", f)?;
    let f = scope.create_function(move |_, (id, on): (EntityArg, bool)| {
        let id = id.live("Scene", "SetStatic")?;
        Ok(scene.borrow_mut().world.set_static(id, on))
    });
    put(table, "SetStatic", f)?;
    let f = scope
        .create_function(move |_, id: EntityArg| Ok(scene.borrow().world.is_static(id.read())));
    put(table, "IsStatic", f)?;
    let f = scope.create_function(|lua, (id, name): (EntityArg, String)| {
        instances::find(lua, id.read(), &name)
    });
    put(table, "GetScript", f)
}
