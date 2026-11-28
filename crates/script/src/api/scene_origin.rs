//! The floating origin: `GetOrigin` gives the metres a shifted World is from authored space,
//! and `SetOriginFocus` picks the entity whose distance from zero decides a shift.
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_scene::Scene;

pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetOrigin",
        lua.create_function(move |lua, ()| {
            let o = scene.get(lua)?.borrow().loaded.origin.offset();
            Ok((o.x, o.y, o.z))
        }),
    )?;
    put(
        table,
        "SetOriginFocus",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let id = id.get().filter(|&id| scene.world.contains(id));
            scene.loaded.origin.focus = id;
            Ok(())
        }),
    )
}
