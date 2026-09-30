//! `Graphics.{Get,Set}Fog*` (#437): the scene's distance + height fog, through the
//! same `authoring::fog` ops the Scene Settings panel calls. Fog is scene-level, not
//! per-volume, so these work whether or not the scene has a visual-correction volume.

use std::cell::RefCell;

use glam::Vec3;

use super::super::{put, Reg};
use crate::scene::authoring::fog as fog_ops;
use crate::scene::{FogMode, FogSettings, Scene};

/// Mode, colour, and the five scalar knobs.
pub(super) fn register_fog<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_mode_color(scope, table, scene)?;
    for (name, get, set) in SCALARS {
        scalar(scope, table, scene, name, get, set)?;
    }
    Ok(())
}

/// `{Get,Set}FogMode` (by name) and `{Get,Set}FogColor` (`r, g, b`).
fn register_mode_color<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetFogMode",
        scope.create_function(|_, ()| Ok(scene.borrow().fog.mode.name())),
    )?;
    put(
        table,
        "SetFogMode",
        scope.create_function(|_, name: String| {
            // An unrecognised name keeps the current mode, like `SetTonemap`.
            if let Some(mode) = FogMode::parse(&name) {
                fog_ops::set_mode(&mut scene.borrow_mut().fog, mode);
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFogColor",
        scope.create_function(|_, ()| {
            let c = scene.borrow().fog.color;
            Ok((c.x, c.y, c.z))
        }),
    )?;
    put(
        table,
        "SetFogColor",
        scope.create_function(|_, (r, g, b): (f32, f32, f32)| {
            fog_ops::set_color(&mut scene.borrow_mut().fog, Vec3::new(r, g, b));
            Ok(())
        }),
    )?;
    Ok(())
}

type Get = fn(&FogSettings) -> f32;
type Set = fn(&mut FogSettings, f32);

/// The five scalar knobs, `GetFog<name>` / `SetFog<name>` each.
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

/// Register `GetFog<name>` / `SetFog<name>` over one scalar field.
fn scalar<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    name: &str,
    get: Get,
    set: Set,
) -> Reg {
    put(
        table,
        &format!("GetFog{name}"),
        scope.create_function(move |_, ()| Ok(get(&scene.borrow().fog))),
    )?;
    put(
        table,
        &format!("SetFog{name}"),
        scope.create_function(move |_, v: f32| {
            set(&mut scene.borrow_mut().fog, v);
            Ok(())
        }),
    )
}
