//! `Graphics.{Get,Set}ColorLookup(path | nil)` (#1441) and its contribution knob
//! `Graphics.{Get,Set}ColorLookupContribution`.

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use crate::scene::authoring::rendering::post_processing::grading::color_lookup;
use crate::scene::Scene;

/// The LUT path (`nil` without one, or without an active volume) and the knob.
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
