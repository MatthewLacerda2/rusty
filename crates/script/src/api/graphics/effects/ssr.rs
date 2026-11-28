use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use super::knobs::{register_count, register_flag, register_quality};
use crate::api::ApiCell;
use rusty_components::{SsrAlgorithm, SsrTracing};
use rusty_edit::authoring::rendering::post_processing::ssr;
use rusty_scene::Scene;

/// A `Graphics.Set<api>(name)` / `Get<api>()` pair over one of the volume's named enums.
macro_rules! register_named {
    ($lua:expr, $table:expr, $scene:expr, $api:literal, $what:literal, $Enum:ty, $get:expr, $set:expr) => {{
        put(
            $table,
            concat!("Set", $api),
            $lua.create_function(move |lua, name: String| {
                let value = <$Enum>::parse(&name).ok_or_else(|| {
                    mlua::Error::RuntimeError(format!(
                        concat!(
                            "Graphics.Set",
                            $api,
                            ": unknown ",
                            $what,
                            " '{}' (one of {})"
                        ),
                        name,
                        <$Enum>::names()
                    ))
                })?;
                with_vc_mut(&*$scene.get(lua)?, |e| $set(e, value));
                Ok(())
            }),
        )?;
        put(
            $table,
            concat!("Get", $api),
            $lua.create_function(move |lua, ()| {
                let value: $Enum = with_vc(&*$scene.get(lua)?, $get).unwrap_or_default();
                Ok(value.name().to_string())
            }),
        )
    }};
}

fn register_names(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_named!(
        lua,
        table,
        scene,
        "SsrTracing",
        "tracing",
        SsrTracing,
        |vc: &rusty_components::PostProcessingComponent| vc.ssr.tracing,
        ssr::set_tracing
    )?;
    register_named!(
        lua,
        table,
        scene,
        "SsrAlgorithm",
        "algorithm",
        SsrAlgorithm,
        |vc: &rusty_components::PostProcessingComponent| vc.ssr.algorithm,
        ssr::set_algorithm
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
    register_names(lua, table, scene)?;
    super::knobs::register(lua, table, scene, ssr::EFFECT.knobs)
}
