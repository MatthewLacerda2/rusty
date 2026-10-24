//! src/api/material/slots.rs — an entity's material slots from Lua (#1285).
//!
//! One slot per sub-mesh run of the entity's mesh, Unity's `Renderer.materials`:
//! - `Get(id)` / `Set(id, key)` — the first slot (Unity's `renderer.sharedMaterial`);
//! - `GetAll(id)` / `SetAt(id, index, key)` — every slot, by 0-based index (Unity's
//!   `sharedMaterials[i]`).
//!
//! Thin adapters over `authoring::material`'s slot ops, which the editor's Material
//! card calls too: a slot out of range or a key the library lacks raises an error
//! naming it, never a panic.

use std::cell::RefCell;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::scene::authoring::material as mat_ops;
use crate::scene::Scene;

/// Register the slot verbs onto the `Material` `table`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "Get",
        scope.create_function(|_, id: EntityArg| {
            let id = id.read();
            Ok(mat_ops::keys(&scene.borrow().world, id).into_iter().next())
        }),
    )?;
    put(
        table,
        "GetAll",
        scope.create_function(|_, id: EntityArg| {
            let id = id.read();
            Ok(mat_ops::keys(&scene.borrow().world, id))
        }),
    )?;
    put(
        table,
        "Set",
        scope.create_function(|_, (id, key): (EntityArg, String)| {
            let id = id.live("Material", "Set")?;
            set(scene, id, 0, &key)
        }),
    )?;
    put(
        table,
        "SetAt",
        scope.create_function(|_, (id, slot, key): (EntityArg, usize, String)| {
            let id = id.live("Material", "SetAt")?;
            set(scene, id, slot, &key)
        }),
    )
}

/// Point slot `slot` of entity `id` at library material `key`.
fn set(scene: &RefCell<Scene>, id: u32, slot: usize, key: &str) -> mlua::Result<()> {
    let mut scene = scene.borrow_mut();
    let scene = &mut *scene;
    mat_ops::set_slot(&mut scene.world, &scene.materials, id, slot, key)
        .map_err(mlua::Error::RuntimeError)
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
