use glam::Vec3;

use super::super::{put, Reg};
use crate::api::ApiCell;
use rusty_edit::authoring::fog as fog_ops;
use rusty_scene::{FogMode, FogSettings, Scene};

pub(super) fn register_fog(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_mode_color(lua, table, scene)?;
    for (name, get, set) in SCALARS {
        scalar(lua, table, scene, name, get, set)?;
    }
    Ok(())
}

fn register_mode_color(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetFogMode",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().fog.mode.name())),
    )?;
    put(
        table,
        "SetFogMode",
        lua.create_function(move |lua, name: String| {
            // An unknown name keeps the current mode, as `SetTonemap` does, rather than erroring.
            if let Some(mode) = FogMode::parse(&name) {
                fog_ops::set_mode(&mut scene.get(lua)?.borrow_mut().fog, mode);
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFogColor",
        lua.create_function(move |lua, ()| {
            let c = scene.get(lua)?.borrow().fog.color;
            Ok((c.x, c.y, c.z))
        }),
    )?;
    put(
        table,
        "SetFogColor",
        lua.create_function(move |lua, (r, g, b): (f32, f32, f32)| {
            fog_ops::set_color(&mut scene.get(lua)?.borrow_mut().fog, Vec3::new(r, g, b));
            Ok(())
        }),
    )?;
    Ok(())
}

type Get = fn(&FogSettings) -> f32;
type Set = fn(&mut FogSettings, f32);

const SCALARS: [(&str, Get, Set); 5] = [
    ("Density", |f| f.density, fog_ops::set_density),
    ("Start", |f| f.start, fog_ops::set_start),
    ("End", |f| f.end, fog_ops::set_end),
    (
        "HeightFalloff",
        |f| f.height_falloff,
        fog_ops::set_height_falloff,
    ),
    ("BaseHeight", |f| f.base_height, fog_ops::set_base_height),
];

fn scalar(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    name: &str,
    get: Get,
    set: Set,
) -> Reg {
    put(
        table,
        &format!("GetFog{name}"),
        lua.create_function(move |lua, ()| Ok(get(&scene.get(lua)?.borrow().fog))),
    )?;
    put(
        table,
        &format!("SetFog{name}"),
        lua.create_function(move |lua, v: f32| {
            set(&mut scene.get(lua)?.borrow_mut().fog, v);
            Ok(())
        }),
    )
}
