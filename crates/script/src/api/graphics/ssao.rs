//! `Graphics.{Get,Set}Ssao{Active,Radius,Intensity}` (#436) and its quality,
//! `Graphics.{Get,Set}Ssao{Quality,Samples,ResolutionDivisor}` (#1522): screen-space
//! ambient occlusion on the active post-processing volume, through the same
//! `authoring::rendering::post_processing` ops the Inspector card calls.

use super::super::{put, Reg};
use super::effects::knobs::{register_count, register_quality};
use super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_components::SsaoSettings;
use rusty_scene::authoring::rendering::post_processing as vc_ops;
use rusty_scene::Scene;

/// Active flag, radius and intensity. AO lives on the volume, so without an active
/// one `GetSsaoActive` reports `false` (nothing runs) and the other getters report
/// the defaults a new volume would start with.
pub(super) fn register_ssao(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetSsaoActive",
        lua.create_function(move |lua, on: bool| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::volume::set_ssao_active(e, on)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsaoActive",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.ssao.active).unwrap_or(false))
        }),
    )?;
    register_ssao_shape(lua, table, scene)?;
    register_ssao_quality(lua, table, scene)
}

/// The occlusion's radius and intensity.
fn register_ssao_shape(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let defaults = SsaoSettings::default();
    put(
        table,
        "SetSsaoRadius",
        lua.create_function(move |lua, radius: f32| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::volume::set_ssao_radius(e, radius)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsaoRadius",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.ssao.radius).unwrap_or(defaults.radius))
        }),
    )?;
    put(
        table,
        "SetSsaoIntensity",
        lua.create_function(move |lua, intensity: f32| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::volume::set_ssao_intensity(e, intensity)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsaoIntensity",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.ssao.intensity).unwrap_or(defaults.intensity))
        }),
    )
}

/// The quality level and its `Custom` sample count and resolution divisor (#1522).
fn register_ssao_quality(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_quality(
        lua,
        (table, scene),
        "SsaoQuality",
        |vc| vc.ssao.quality,
        vc_ops::volume::set_ssao_quality,
    )?;
    register_count(
        lua,
        (table, scene),
        "SsaoSamples",
        |vc| vc.ssao.samples,
        vc_ops::volume::set_ssao_samples,
    )?;
    register_count(
        lua,
        (table, scene),
        "SsaoResolutionDivisor",
        |vc| vc.ssao.resolution_divisor,
        vc_ops::volume::set_ssao_resolution_divisor,
    )
}
