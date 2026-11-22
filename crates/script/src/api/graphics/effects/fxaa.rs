use super::super::state::{with_cam, with_cam_mut};
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_edit::authoring::rendering::camera as camera_ops;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetFxaaActive",
        lua.create_function(move |lua, active: bool| {
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_fxaa_active(e, active)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFxaaActive",
        lua.create_function(move |lua, ()| {
            Ok(with_cam(&*scene.get(lua)?, |c| c.fxaa_active).unwrap_or(true))
        }),
    )
}
