//! `Graphics.{Get,Set}WhiteBalance(temperature, tint)`: both knobs in one pair.

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use crate::scene::authoring::rendering::post_processing::grading::white_balance;
use crate::scene::Scene;

/// The pair; neutral without an active volume, as a new volume starts.
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    put(
        table,
        "SetWhiteBalance",
        lua.create_function(move |lua, (temperature, tint): (f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                white_balance::set(e, temperature, tint)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetWhiteBalance",
        lua.create_function(move |lua, ()| {
            let wb = with_vc(&*scene.get(lua)?, |vc| vc.white_balance).unwrap_or_default();
            Ok((wb.temperature, wb.tint))
        }),
    )
}
