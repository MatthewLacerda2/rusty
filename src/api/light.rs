//! src/api/light.rs — `Light` namespace.
//!
//! Get/Set over an entity's optional `LightComponent`: colour, intensity, range,
//! light type, whether it casts shadows, its mode (#438) and an Area light's size
//! (#1674). Every setter maps onto a
//! real field the renderer reads
//! (`apply_scene_lights`), so a script tweak takes effect on the next frame's
//! lighting uniform; an Area light is baked only, so its size shows after the
//! next bake. There is no per-light "active" flag in the engine — a light
//! is gated by its owning entity's `active` (a `Scene` concern), so this surface
//! deliberately exposes the light's own data and nothing more.

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::{LightMode, LightType};
use crate::scene::authoring::rendering::light as light_ops;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Light` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>, console: ApiCell<ConsoleLogs>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_color(lua, &table, scene)?;
    register_intensity(lua, &table, scene)?;
    register_range(lua, &table, scene)?;
    register_type(lua, &table, scene)?;
    register_cast_shadows(lua, &table, scene)?;
    register_mode(lua, &table, scene, console)?;
    register_area_size(lua, &table, scene)?;

    lua.globals().set("Light", table).map_err(|e| e.to_string())
}

/// `GetColor` / `SetColor` over the light's linear RGB colour.
fn register_color(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetColor",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let c = scene.world.light(id.read()).map(|l| l.color);
            let c = c.unwrap_or(Vec3::ONE);
            Ok((c.x, c.y, c.z))
        }),
    )?;

    put(
        table,
        "SetColor",
        lua.create_function(move |lua, (id, r, g, b): (EntityArg, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
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
fn register_intensity(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetIntensity",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
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
        lua.create_function(move |lua, (id, value): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
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
fn register_range(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetRange",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.light(id.read()).map(|l| l.range).unwrap_or(0.0))
        }),
    )?;

    put(
        table,
        "SetRange",
        lua.create_function(move |lua, (id, value): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
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
fn register_type(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetType",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
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
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
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
fn register_cast_shadows(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetCastShadows",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.light(id.read()).is_some_and(|l| l.cast_shadows))
        }),
    )?;

    put(
        table,
        "SetCastShadows",
        lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
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
/// light reads its effective `Baked` and refuses anything else with a console
/// warning (#1674).
fn register_mode(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "GetMode",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let mode = scene
                .world
                .light(id.read())
                .map(|l| l.effective_mode().name());
            Ok(mode.unwrap_or("None").to_string())
        }),
    )?;

    put(
        table,
        "SetMode",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            scene.with(lua, |scene| {
                let id = id.live("Light", "SetMode")?;
                let mut scene = scene.borrow_mut();
                if let (Some(mut c), Some(mode)) =
                    (scene.world.light_mut(id), LightMode::parse(&name))
                {
                    if !light_ops::set_mode(&mut c, mode) {
                        console.get(lua)?.borrow_mut().warn(format!(
                            "Light.SetMode: an Area light is always Baked, \"{}\" refused",
                            mode.name()
                        ));
                    }
                }
                Ok(())
            })
        }),
    )
}

/// `GetAreaSize` / `SetAreaSize`: an Area light's width and height, Unity's
/// `areaSize` (#1674). Any light carries one; only an Area light uses it.
fn register_area_size(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetAreaSize",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let size = scene.world.light(id.read()).map(|l| l.area_size);
            let size = size.unwrap_or(glam::Vec2::ZERO);
            Ok((size.x, size.y))
        }),
    )?;

    put(
        table,
        "SetAreaSize",
        lua.create_function(move |lua, (id, w, h): (EntityArg, f32, f32)| {
            let scene = &*scene.get(lua)?;
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
