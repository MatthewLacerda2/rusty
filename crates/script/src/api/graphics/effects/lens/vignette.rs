//! `Graphics.{Get,Set}Vignette{Active,Intensity,Smoothness,Color,Center}` (#1439).

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use super::super::knobs::{register as knobs, register_flag};
use crate::api::ApiCell;
use rusty_components::Vignette;
use rusty_scene::authoring::rendering::post_processing::lens::vignette;
use rusty_scene::Scene;

/// The switch, the knobs, and the colour and centre (a new volume's without an
/// active volume).
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    let set = vignette::set_active;
    register_flag(
        lua,
        (table, scene),
        "VignetteActive",
        |vc| vc.vignette.active,
        set,
    )?;
    knobs(lua, table, scene, vignette::EFFECT.knobs)?;
    register_vectors(lua, table, scene)
}

/// `SetVignetteColor(r, g, b)` / `GetVignetteColor() -> r, g, b` and
/// `SetVignetteCenter(x, y)` / `GetVignetteCenter() -> x, y`.
fn register_vectors(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let defaults = Vignette::DEFAULT;
    put(
        table,
        "SetVignetteColor",
        lua.create_function(move |lua, (r, g, b): (f32, f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| vignette::set_color(e, [r, g, b]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVignetteColor",
        lua.create_function(move |lua, ()| {
            let [r, g, b] =
                with_vc(&*scene.get(lua)?, |vc| vc.vignette.color).unwrap_or(defaults.color);
            Ok((r, g, b))
        }),
    )?;
    put(
        table,
        "SetVignetteCenter",
        lua.create_function(move |lua, (x, y): (f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| vignette::set_center(e, [x, y]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVignetteCenter",
        lua.create_function(move |lua, ()| {
            let [x, y] =
                with_vc(&*scene.get(lua)?, |vc| vc.vignette.center).unwrap_or(defaults.center);
            Ok((x, y))
        }),
    )
}
