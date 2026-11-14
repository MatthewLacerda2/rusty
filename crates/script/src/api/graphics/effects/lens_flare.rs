//! `Graphics.{Get,Set}LensFlare{Active,Intensity,GhostCount,GhostIntensity,
//! HaloIntensity,StreakIntensity,Tint,ChromaticAberration}` (#1443).

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use super::knobs::{register as knobs, register_flag};
use crate::api::ApiCell;
use rusty_components::rendering::post_processing::LensFlare;
use rusty_scene::authoring::rendering::post_processing::lens_flare;
use rusty_scene::Scene;

/// The switch, the knobs, the ghost count and the tint (a new volume's without an
/// active volume).
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let set = lens_flare::set_active;
    register_flag(
        lua,
        (table, scene),
        "LensFlareActive",
        |vc| vc.lens_flare.active,
        set,
    )?;
    knobs(lua, table, scene, lens_flare::EFFECT.knobs)?;
    register_count_and_tint(lua, table, scene)
}

/// `SetLensFlareGhostCount(n)` / `GetLensFlareGhostCount() -> n` and
/// `SetLensFlareTint(r, g, b)` / `GetLensFlareTint() -> r, g, b`.
fn register_count_and_tint(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let defaults = LensFlare::DEFAULT;
    put(
        table,
        "SetLensFlareGhostCount",
        lua.create_function(move |lua, n: u32| {
            with_vc_mut(&*scene.get(lua)?, |e| lens_flare::set_ghost_count(e, n));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetLensFlareGhostCount",
        lua.create_function(move |lua, ()| {
            let n = with_vc(&*scene.get(lua)?, |vc| vc.lens_flare.ghost_count);
            Ok(n.unwrap_or(defaults.ghost_count))
        }),
    )?;
    put(
        table,
        "SetLensFlareTint",
        lua.create_function(move |lua, (r, g, b): (f32, f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| lens_flare::set_tint(e, [r, g, b]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetLensFlareTint",
        lua.create_function(move |lua, ()| {
            let [r, g, b] =
                with_vc(&*scene.get(lua)?, |vc| vc.lens_flare.tint).unwrap_or(defaults.tint);
            Ok((r, g, b))
        }),
    )
}
