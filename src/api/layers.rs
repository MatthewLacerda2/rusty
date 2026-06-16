//! src/api/layers.rs — `Layers` namespace.
//!
//! Read/write an entity's layer index and resolve layer names against the
//! project's shared registry (Unity's `gameObject.layer` + `LayerMask.NameToLayer`).
//! Groundwork (#90): this only moves an entity between named slots — it does not
//! change collision or rendering yet (#91/#92 build on it).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Layers` namespace onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetLayer",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            Ok(scene.get_entity(id).map(|e| e.layer).unwrap_or(0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetLayer",
        lua.create_function(move |_, (id, layer): (u32, u8)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.layer = layer;
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetName",
        lua.create_function(move |_, index: u8| {
            let scene = s.borrow();
            Ok(scene.layers.label(index))
        }),
    )?;

    // Unity's `LayerMask.NameToLayer`: the index for a layer name, or `nil`.
    let s = Rc::clone(scene);
    put(
        &table,
        "NameToIndex",
        lua.create_function(move |_, name: String| {
            let scene = s.borrow();
            Ok(scene.layers.index_of(&name))
        }),
    )?;

    lua.globals()
        .set("Layers", table)
        .map_err(|e| e.to_string())
}
