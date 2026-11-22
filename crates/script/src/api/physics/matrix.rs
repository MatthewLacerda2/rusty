use crate::api::layers::check_index;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_scene::Scene;

pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "IgnoreLayerCollision",
        lua.create_function(move |lua, (a, b, ignore): (u8, u8, Option<bool>)| {
            let scene = &*scene.get(lua)?;
            check_index(a)?;
            check_index(b)?;
            let collide = !ignore.unwrap_or(true);
            scene
                .borrow_mut()
                .collision_matrix
                .set_collision(a, b, collide);
            Ok(())
        }),
    )?;

    put(
        table,
        "GetIgnoreLayerCollision",
        lua.create_function(move |lua, (a, b): (u8, u8)| {
            check_index(a)?;
            check_index(b)?;
            Ok(!scene.get(lua)?.borrow().collision_matrix.can_collide(a, b))
        }),
    )
}
