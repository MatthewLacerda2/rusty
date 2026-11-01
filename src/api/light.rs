//! src/api/light.rs — `Light` namespace.
//!
//! Get/Set over an entity's optional `LightComponent`: colour, intensity, range,
//! light type, whether it casts shadows, its mode (#438) and an Area light's size
//! (#1674). Every setter maps onto a
//! real field the renderer reads
//! (`apply_scene_lights`), so a script tweak takes effect on the next frame's
//! lighting uniform. There is no per-light "active" flag in the engine — a light
//! is gated by its owning entity's `active` (a `Scene` concern), so this surface
//! deliberately exposes the light's own data and nothing more.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::components::{LightMode, LightType};
use crate::scene::authoring::rendering::light as light_ops;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Light` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_color(scope, &table, scene)?;
    register_intensity(scope, &table, scene)?;
    register_range(scope, &table, scene)?;
    register_type(scope, &table, scene)?;
    register_cast_shadows(scope, &table, scene)?;
    register_mode(scope, &table, scene, console)?;
    register_area_size(scope, &table, scene)?;

    lua.globals().set("Light", table).map_err(|e| e.to_string())
}

/// `GetColor` / `SetColor` over the light's linear RGB colour.
fn register_color<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetColor",
        scope.create_function(|_, id: EntityArg| {
            let scene = scene.borrow();
            let c = scene.world.light(id.read()).map(|l| l.color);
            let c = c.unwrap_or(Vec3::ONE);
            Ok((c.x, c.y, c.z))
        }),
    )?;

    put(
        table,
        "SetColor",
        scope.create_function(|_, (id, r, g, b): (EntityArg, f32, f32, f32)| {
            let id = id.live("Light", "SetColor")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                light_ops::set_color(&mut c, Vec3::new(r, g, b));
            }
            Ok(())
        }),
    )
}

/// `GetIntensity` / `SetIntensity` (clamped ≥ 0).
fn register_intensity<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetIntensity",
        scope.create_function(|_, id: EntityArg| {
            let scene = scene.borrow();
            Ok(scene
                .world
                .light(id.read())
                .map(|l| l.intensity)
                .unwrap_or(0.0))
        }),
    )?;

    put(
        table,
        "SetIntensity",
        scope.create_function(|_, (id, value): (EntityArg, f32)| {
            let id = id.live("Light", "SetIntensity")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                light_ops::set_intensity(&mut c, value);
            }
            Ok(())
        }),
    )
}

/// `GetRange` / `SetRange` (clamped ≥ 0).
fn register_range<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetRange",
        scope.create_function(|_, id: EntityArg| {
            let scene = scene.borrow();
            Ok(scene.world.light(id.read()).map(|l| l.range).unwrap_or(0.0))
        }),
    )?;

    put(
        table,
        "SetRange",
        scope.create_function(|_, (id, value): (EntityArg, f32)| {
            let id = id.live("Light", "SetRange")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                light_ops::set_range(&mut c, value);
            }
            Ok(())
        }),
    )
}

/// `GetType` / `SetType` over the light's kind, by name. Unknown names on `SetType`
/// are ignored (the current type is kept).
fn register_type<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetType",
        scope.create_function(|_, id: EntityArg| {
            let scene = scene.borrow();
            let name = scene
                .world
                .light(id.read())
                .map(|l| l.light_type.name())
                .unwrap_or("None");
            Ok(name.to_string())
        }),
    )?;

    put(
        table,
        "SetType",
        scope.create_function(|_, (id, name): (EntityArg, String)| {
            let id = id.live("Light", "SetType")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                if let Some(t) = LightType::parse(&name) {
                    light_ops::set_type(&mut c, t);
                }
            }
            Ok(())
        }),
    )
}

/// `GetCastShadows` / `SetCastShadows`: whether a point or spot light casts shadows
/// through the shadow atlas (#468). The sun always casts; it ignores the flag.
fn register_cast_shadows<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetCastShadows",
        scope.create_function(|_, id: EntityArg| {
            let scene = scene.borrow();
            Ok(scene.world.light(id.read()).is_some_and(|l| l.cast_shadows))
        }),
    )?;

    put(
        table,
        "SetCastShadows",
        scope.create_function(|_, (id, on): (EntityArg, bool)| {
            let id = id.live("Light", "SetCastShadows")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                light_ops::set_cast_shadows(&mut c, on);
            }
            Ok(())
        }),
    )
}

/// `GetMode` / `SetMode` over the light's Realtime / Mixed / Baked mode (#438), by
/// name. Unknown names on `SetMode` are ignored (the current mode is kept); an Area
/// light refuses anything but `Baked` with a console warning (#1674).
fn register_mode<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "GetMode",
        scope.create_function(|_, id: EntityArg| {
            let scene = scene.borrow();
            let mode = scene.world.light(id.read()).map(|l| l.mode.name());
            Ok(mode.unwrap_or("None").to_string())
        }),
    )?;

    put(
        table,
        "SetMode",
        scope.create_function(|_, (id, name): (EntityArg, String)| {
            let id = id.live("Light", "SetMode")?;
            let mut scene = scene.borrow_mut();
            if let (Some(mut c), Some(mode)) = (scene.world.light_mut(id), LightMode::parse(&name))
            {
                if !light_ops::set_mode(&mut c, mode) {
                    console.borrow_mut().warn(format!(
                        "Light.SetMode: an Area light is always Baked, \"{}\" refused",
                        mode.name()
                    ));
                }
            }
            Ok(())
        }),
    )
}

/// `GetAreaSize` / `SetAreaSize`: an Area light's width and height, Unity's
/// `areaSize` (#1674). Any light carries one; only an Area light uses it.
fn register_area_size<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetAreaSize",
        scope.create_function(|_, id: EntityArg| {
            let scene = scene.borrow();
            let size = scene.world.light(id.read()).map(|l| l.area_size);
            let size = size.unwrap_or(glam::Vec2::ZERO);
            Ok((size.x, size.y))
        }),
    )?;

    put(
        table,
        "SetAreaSize",
        scope.create_function(|_, (id, w, h): (EntityArg, f32, f32)| {
            let id = id.live("Light", "SetAreaSize")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                light_ops::set_area_size(&mut c, glam::Vec2::new(w, h));
            }
            Ok(())
        }),
    )
}

#[cfg(test)]
#[path = "light_tests.rs"]
mod light_tests;
