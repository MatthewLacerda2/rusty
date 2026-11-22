use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_components::ExposureMode;
use rusty_edit::authoring::rendering::post_processing as vc_ops;
use rusty_scene::Scene;

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
