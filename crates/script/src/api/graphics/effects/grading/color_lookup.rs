use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_edit::authoring::rendering::post_processing::grading::color_lookup;
use rusty_scene::Scene;

pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    put(
        table,
        "SetColorLookup",
        lua.create_function(move |lua, path: Option<String>| {
            with_vc_mut(&*scene.get(lua)?, |e| color_lookup::set_lut(e, path));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetColorLookup",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.color_lookup.lut.clone()).flatten())
        }),
    )?;
    super::super::knobs::register(lua, table, scene, color_lookup::EFFECT.knobs)
}
