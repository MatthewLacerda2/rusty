//! `Graphics.{Get,Set}Exposure`, `{Get,Set}ExposureMode` and the automatic mode's
//! knobs (#1444): limits, compensation, adaptation speeds and histogram window, on
//! the active post-processing volume, through the same
//! `authoring::rendering::post_processing` ops the Inspector card calls.

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use crate::components::ExposureMode;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// The mode, the automatic mode's paired knobs, and the knobs on their own (the
/// fixed EV and the compensation). Without an active volume the getters report what
/// a new volume starts with. `GetExposure` is the configured EV in both modes, never
/// the adapted one: scripts run in the sim, and the adapted EV is a GPU readback
/// (the dev-only `Debug.GetAdaptedExposure` reads that).
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetExposureMode",
        lua.create_function(move |lua, name: String| {
            if let Some(mode) = ExposureMode::parse(&name) {
                with_vc_mut(&*scene.get(lua)?, |e| vc_ops::exposure::set_mode(e, mode));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposureMode",
        lua.create_function(move |lua, ()| {
            let mode = with_vc(&*scene.get(lua)?, |vc| vc.exposure.mode).unwrap_or_default();
            Ok(mode.name().to_string())
        }),
    )?;
    register_limits(lua, table, scene)?;
    register_adaptation(lua, table, scene)?;
    super::knobs::register(lua, table, scene, vc_ops::exposure::EFFECT.knobs)
}

/// The automatic mode's limits.
fn register_limits(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let auto = move |lua: &mlua::Lua| -> mlua::Result<_> {
        Ok(with_vc(&*scene.get(lua)?, |vc| vc.exposure.automatic).unwrap_or_default())
    };
    put(
        table,
        "SetExposureLimits",
        lua.create_function(move |lua, (min, max): (f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::exposure::set_limits(e, min, max)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposureLimits",
        lua.create_function(move |lua, ()| {
            let a = auto(lua)?;
            Ok((a.min_ev, a.max_ev))
        }),
    )
}

/// The automatic mode's adaptation speeds and histogram window.
fn register_adaptation(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let auto = move |lua: &mlua::Lua| -> mlua::Result<_> {
        Ok(with_vc(&*scene.get(lua)?, |vc| vc.exposure.automatic).unwrap_or_default())
    };
    put(
        table,
        "SetAdaptationSpeed",
        lua.create_function(move |lua, (up, down): (f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::exposure::set_adaptation_speed(e, up, down)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetAdaptationSpeed",
        lua.create_function(move |lua, ()| {
            let a = auto(lua)?;
            Ok((a.speed_up, a.speed_down))
        }),
    )?;
    put(
        table,
        "SetHistogramPercent",
        lua.create_function(move |lua, (low, high): (f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::exposure::set_histogram_percent(e, low, high)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetHistogramPercent",
        lua.create_function(move |lua, ()| {
            let a = auto(lua)?;
            Ok((a.low_percent, a.high_percent))
        }),
    )
}
