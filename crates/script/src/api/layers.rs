use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::Scene;
use rusty_scene::LAYER_COUNT;

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetLayer",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(scene.get(lua)?.borrow().world.layer(id.read()))
        }),
    )?;

    put(
        &table,
        "SetLayer",
        lua.create_function(move |lua, (id, layer): (EntityArg, u8)| {
            let id = id.live("Layers", "SetLayer")?;
            scene.get(lua)?.borrow_mut().world.set_layer(id, layer);
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetName",
        lua.create_function(move |lua, index: u8| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.layers.label(index))
        }),
    )?;

    put(
        &table,
        "SetName",
        lua.create_function(move |lua, (index, name): (u8, String)| {
            check_index(index)?;
            scene.get(lua)?.borrow_mut().layers.set_name(index, name);
            Ok(())
        }),
    )?;

    // Unity's `LayerMask.NameToLayer`: the index for a layer name, or `nil`.
    put(
        &table,
        "NameToIndex",
        lua.create_function(move |lua, name: String| {
            let scene = &*scene.get(lua)?;
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
