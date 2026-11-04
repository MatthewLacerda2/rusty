//! The `Graphics.Get/Set<api>` pair of every effect knob that is one on its own
//! (#1467): each effect's knob table (`authoring::rendering::post_processing`)
//! names the pair, so the functions, the Inspector's sliders and the
//! `Graphics.<effect>.<knob>` tween paths reach the same value through one op.

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use crate::components::PostProcessingComponent;
use crate::core::settings::quality::EffectQuality;
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing::Knob;
use crate::scene::Scene;

/// Register `Set<api>(value)` / `Get<api>()` for each of `knobs` that names an
/// `api`. Without an active volume the setter does nothing and the getter reports
/// what a new volume starts with.
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    knobs: &'static [Knob],
) -> Reg {
    for knob in knobs {
        let Some(api) = knob.api else { continue };
        put(
            table,
            &format!("Set{api}"),
            lua.create_function(move |lua, value: f32| {
                with_vc_mut(&*scene.get(lua)?, |vc| (knob.set)(vc, value));
                Ok(())
            }),
        )?;
        put(
            table,
            &format!("Get{api}"),
            lua.create_function(move |lua, ()| {
                let value = with_vc(&*scene.get(lua)?, knob.get);
                Ok(value.unwrap_or_else(|| (knob.get)(&default_post_processing())))
            }),
        )?;
    }
    Ok(())
}

/// Register `Set<name>(bool)` / `Get<name>()` over an effect's switch: `false`
/// without an active volume, since nothing runs then.
pub(in crate::api::graphics) fn register_flag(
    lua: &mlua::Lua,
    (table, scene): (&mlua::Table, ApiCell<Scene>),
    name: &str,
    get: fn(&PostProcessingComponent) -> bool,
    set: fn(&mut PostProcessingComponent, bool),
) -> Reg {
    put(
        table,
        &format!("Set{name}"),
        lua.create_function(move |lua, on: bool| {
            with_vc_mut(&*scene.get(lua)?, |vc| set(vc, on));
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}"),
        lua.create_function(move |lua, ()| Ok(with_vc(&*scene.get(lua)?, get).unwrap_or(false))),
    )
}

/// `name` as a quality level, or `Graphics.<function>`'s refusal listing the levels.
pub(in crate::api::graphics) fn parse_quality(
    function: &str,
    name: &str,
) -> mlua::Result<EffectQuality> {
    EffectQuality::parse(name).ok_or_else(|| {
        mlua::Error::RuntimeError(format!(
            "Graphics.{function}: unknown quality '{name}' (one of {})",
            EffectQuality::names()
        ))
    })
}

/// Register `Set<name>(level)` / `Get<name>()` over an effect's quality level
/// (#1522): `"Low"`, `"Medium"`, `"High"` or `"Custom"`, an unknown one refused.
/// Without an active volume the getter reports a new volume's level.
pub(in crate::api::graphics) fn register_quality(
    lua: &mlua::Lua,
    (table, scene): (&mlua::Table, ApiCell<Scene>),
    name: &'static str,
    get: fn(&PostProcessingComponent) -> EffectQuality,
    set: fn(&mut PostProcessingComponent, EffectQuality),
) -> Reg {
    put(
        table,
        &format!("Set{name}"),
        lua.create_function(move |lua, level: String| {
            let quality = parse_quality(&format!("Set{name}"), &level)?;
            with_vc_mut(&*scene.get(lua)?, |vc| set(vc, quality));
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}"),
        lua.create_function(move |lua, ()| {
            let quality =
                with_vc(&*scene.get(lua)?, get).unwrap_or_else(|| get(&default_post_processing()));
            Ok(quality.name())
        }),
    )
}

/// Register `Set<name>(n)` / `Get<name>()` over a whole-number setting of the
/// volume (a sample or step count, a resolution divisor: no tween), through its
/// op. Without an active volume the getter reports a new volume's value.
pub(in crate::api::graphics) fn register_count(
    lua: &mlua::Lua,
    (table, scene): (&mlua::Table, ApiCell<Scene>),
    name: &'static str,
    get: fn(&PostProcessingComponent) -> u32,
    set: fn(&mut PostProcessingComponent, u32),
) -> Reg {
    put(
        table,
        &format!("Set{name}"),
        lua.create_function(move |lua, n: u32| {
            with_vc_mut(&*scene.get(lua)?, |vc| set(vc, n));
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}"),
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, get).unwrap_or_else(|| get(&default_post_processing())))
        }),
    )
}
