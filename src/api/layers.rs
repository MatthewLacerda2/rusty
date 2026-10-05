//! src/api/layers.rs — `Layers` namespace.
//!
//! Read/write an entity's layer index and resolve layer names against the
//! project's shared registry (Unity's `gameObject.layer` + `LayerMask.NameToLayer`),
//! and name a slot (#827) — the same `Layers::set_name` the editor's Tags & Layers
//! section commits through. The collision matrix lives on `Physics` and a camera's
//! culling mask on `Camera`, where Unity puts them.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;
use crate::scene::LAYER_COUNT;

/// Register the `Layers` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
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

    // Slot 0 stays the fixed "Default" (a no-op); a blank name clears the slot.
    put(
        &table,
        "SetName",
        scope.create_function(|_, (index, name): (u8, String)| {
            check_index(index)?;
            scene.borrow_mut().layers.set_name(index, name);
            Ok(())
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

/// Fail a layer index outside `0..LAYER_COUNT`, the way Unity's layer calls
/// throw on one, so a typo errors instead of silently writing nothing.
pub(crate) fn check_index(index: u8) -> mlua::Result<()> {
    if (index as usize) < LAYER_COUNT {
        Ok(())
    } else {
        Err(mlua::Error::RuntimeError(format!(
            "layer {index} is out of range (0..{})",
            LAYER_COUNT - 1
        )))
    }
}
