//! src/api/layers.rs — `Layers` namespace.
//!
//! Read/write an entity's layer index and resolve layer names against the
//! project's shared registry (Unity's `gameObject.layer` + `LayerMask.NameToLayer`).
//! Groundwork (#90): this only moves an entity between named slots — it does not
//! change collision or rendering yet (#91/#92 build on it).

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Layers` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetLayer",
        scope.create_function(|_, id: u32| Ok(scene.borrow().world.layer(id))),
    )?;

    put(
        &table,
        "SetLayer",
        scope.create_function(|_, (id, layer): (u32, u8)| {
            scene.borrow_mut().world.set_layer(id, layer);
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetName",
        scope.create_function(|_, index: u8| {
            let scene = scene.borrow();
            Ok(scene.layers.label(index))
        }),
    )?;

    // Unity's `LayerMask.NameToLayer`: the index for a layer name, or `nil`.
    put(
        &table,
        "NameToIndex",
        scope.create_function(|_, name: String| {
            let scene = scene.borrow();
            Ok(scene.layers.index_of(&name))
        }),
    )?;

    lua.globals()
        .set("Layers", table)
        .map_err(|e| e.to_string())
}
