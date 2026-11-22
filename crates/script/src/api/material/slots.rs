use std::cell::RefCell;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_edit::authoring::material as mat_ops;
use rusty_scene::Scene;

pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Get",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(mat_ops::keys(&scene.get(lua)?.borrow().world, id.read())
                .into_iter()
                .next())
        }),
    )?;
    put(
        table,
        "GetAll",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(mat_ops::keys(&scene.get(lua)?.borrow().world, id.read()))
        }),
    )?;
    put(
        table,
        "Set",
        lua.create_function(move |lua, (id, key): (EntityArg, String)| {
            let id = id.live("Material", "Set")?;
            set(&*scene.get(lua)?, id, 0, &key)
        }),
    )?;
    put(
        table,
        "SetAt",
        lua.create_function(move |lua, (id, slot, key): (EntityArg, usize, String)| {
            let id = id.live("Material", "SetAt")?;
            set(&*scene.get(lua)?, id, slot, &key)
        }),
    )
}

fn set(scene: &RefCell<Scene>, id: u32, slot: usize, key: &str) -> mlua::Result<()> {
    let mut scene = scene.borrow_mut();
    let scene = &mut *scene;
    mat_ops::set_slot(&mut scene.world, &scene.materials, id, slot, key)
        .map_err(mlua::Error::RuntimeError)
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
