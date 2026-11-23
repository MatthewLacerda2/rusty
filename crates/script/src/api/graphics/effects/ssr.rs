use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use super::knobs::{register_count, register_flag, register_quality};
use crate::api::ApiCell;
use rusty_components::SsrTracing;
use rusty_edit::authoring::rendering::post_processing::ssr;
use rusty_scene::Scene;

fn register_tracing(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetSsrTracing",
        lua.create_function(move |lua, name: String| {
            let tracing = SsrTracing::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "Graphics.SetSsrTracing: unknown tracing '{name}' (one of {})",
                    SsrTracing::names()
                ))
            })?;
            with_vc_mut(&*scene.get(lua)?, |e| ssr::set_tracing(e, tracing));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsrTracing",
        lua.create_function(move |lua, ()| {
            let tracing = with_vc(&*scene.get(lua)?, |vc| vc.ssr.tracing).unwrap_or_default();
            Ok(tracing.name().to_string())
        }),
    )
}

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_flag(
        lua,
        (table, scene),
        "SsrActive",
        |vc| vc.ssr.active,
        ssr::set_active,
    )?;
    register_quality(
        lua,
        (table, scene),
        "SsrQuality",
        |vc| vc.ssr.quality,
        ssr::set_quality,
    )?;
    register_count(
        lua,
        (table, scene),
        "SsrSteps",
        |vc| vc.ssr.steps,
        ssr::set_steps,
    )?;
    register_count(
        lua,
        (table, scene),
        "SsrTraceResolutionDivisor",
        |vc| vc.ssr.trace_resolution_divisor,
        ssr::set_trace_resolution_divisor,
    )?;
    register_tracing(lua, table, scene)?;
    super::knobs::register(lua, table, scene, ssr::EFFECT.knobs)
}
