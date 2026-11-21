use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_components::PostProcessingComponent;
use rusty_core::settings::quality::EffectQuality;
use rusty_edit::authoring::default_post_processing;
use rusty_edit::authoring::rendering::post_processing::Knob;
use rusty_scene::Scene;

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
