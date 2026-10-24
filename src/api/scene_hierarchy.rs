//! src/api/scene_hierarchy.rs — the `Scene` namespace's hierarchy reads (#422).
//!
//! Registered onto the SAME `Scene` table as `api::scene`. Unity's `transform.parent`,
//! `transform.GetChild` / `childCount`, `transform.Find` and `GetComponent<T>()` for
//! script components — what a widget script needs to reach its own parts (a
//! slider's handle, a toggle's checkmark) without storing ids that a prefab stamp
//! would invalidate:
//!
//!   - `GetParent(id)` — the parent's id, or `nil`.
//!   - `GetChildren(id)` — the child ids in hierarchy order (an empty array for none).
//!   - `FindChild(id, path)` — a descendant by name path (`"Viewport/Content"`),
//!     one level per segment, the first match in hierarchy order; `nil` if absent.
//!   - `SetActive(id, bool)` / `IsActive(id)` — Unity's `SetActive` / `activeSelf`:
//!     the entity's own flag (an inactive ancestor still hides it).
//!     `IsActiveInHierarchy(id)` — Unity's `activeInHierarchy`: it and every
//!     ancestor are active, the rule every system follows (#1363). A pooled
//!     object comes back with `SetActive(id, true)`; `Deactivate` is the old
//!     one-way soft delete.
//!   - `GetScript(id, name)` — entity `id`'s live script instance named `name` (the
//!     file stem, `"button"`), the very table its callbacks run on; `nil` outside
//!     play or when it has none.
//!
//! A `nil` id (a failed `FindEntityByName`) is not an error for the reads: `GetParent`,
//! `FindChild` and `GetScript` answer `nil`, `GetChildren` an empty array and
//! `IsActive` `false`, as Unity's `GetComponent` on a missing object does.

use std::cell::RefCell;

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::instances;

/// Register the hierarchy reads onto the (already-created) `Scene` `table`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(move |_, id: Option<u32>| {
        Ok(id.and_then(|id| scene.borrow().world.parent_id(id)))
    });
    put(table, "GetParent", f)?;
    let f = scope.create_function(move |_, id: Option<u32>| {
        Ok(id.map_or_else(Vec::new, |id| scene.borrow().world.children(id)))
    });
    put(table, "GetChildren", f)?;
    let f = scope.create_function(move |_, (id, path): (Option<u32>, String)| {
        let world = &scene.borrow().world;
        let mut at = id.filter(|&id| world.contains(id));
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            at = at.and_then(|p| {
                world
                    .children(p)
                    .into_iter()
                    .find(|&c| world.name(c).is_some_and(|n| *n == segment))
            });
        }
        Ok(at)
    });
    put(table, "FindChild", f)?;
    let f = scope.create_function(move |_, (id, on): (u32, bool)| {
        Ok(scene.borrow_mut().world.set_active(id, on))
    });
    put(table, "SetActive", f)?;
    // lint: allow(active-self) `IsActive` is Unity's `activeSelf`
    let f = scope.create_function(move |_, id: Option<u32>| {
        Ok(id.is_some_and(|id| scene.borrow().world.is_active_self(id)))
    });
    put(table, "IsActive", f)?;
    let f = scope.create_function(move |_, id: Option<u32>| {
        Ok(id.is_some_and(|id| scene.borrow().world.is_active_in_hierarchy(id)))
    });
    put(table, "IsActiveInHierarchy", f)?;
    let f = scope.create_function(|lua, (id, name): (Option<u32>, String)| match id {
        Some(id) => instances::find(lua, id, &name),
        None => Ok(None),
    });
    put(table, "GetScript", f)
}
