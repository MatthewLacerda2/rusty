mod shadows;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::ConsoleLogs;
use rusty_components::{LightMode, LightType};
use rusty_edit::authoring::rendering::light as light_ops;
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>, console: ApiCell<ConsoleLogs>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_color(lua, &table, scene)?;
    register_intensity(lua, &table, scene)?;
    register_range(lua, &table, scene)?;
    register_type(lua, &table, scene)?;
    shadows::register(lua, &table, scene)?;
    register_mode(lua, &table, scene, console)?;
    register_area_size(lua, &table, scene)?;

    lua.globals().set("Light", table).map_err(|e| e.to_string())
}

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
mod tests;
