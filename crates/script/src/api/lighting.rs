use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_edit::authoring::environment as env_ops;
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_skybox(lua, &table, scene)?;
    register_ambient(lua, &table, scene)?;
    register_ambient_gradient(lua, &table, scene)?;
    register_sky_colors(lua, &table, scene)?;
    register_clouds(lua, &table, scene)?;
    lua.globals()
        .set("Lighting", table)
        .map_err(|e| e.to_string())
}

fn register_skybox(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetSkybox",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().skybox_path.clone())),
    )?;
    put(
        table,
        "SetSkybox",
        lua.create_function(move |lua, path: Option<String>| {
            env_ops::set_skybox(
                &mut scene.get(lua)?.borrow_mut(),
                path.as_deref().unwrap_or(""),
            );
            Ok(())
        }),
    )
}

fn register_ambient(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetAmbientColor",
        lua.create_function(move |lua, ()| {
            let c = scene.get(lua)?.borrow().ambient_color;
            Ok((c.x, c.y, c.z))
        }),
    )?;
    put(
        table,
        "SetAmbientColor",
        lua.create_function(move |lua, (r, g, b): (f32, f32, f32)| {
            env_ops::set_ambient_color(&mut scene.get(lua)?.borrow_mut(), Vec3::new(r, g, b));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetAmbientIntensity",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().ambient_intensity)),
    )?;
    put(
        table,
        "SetAmbientIntensity",
        lua.create_function(move |lua, v: f32| {
            env_ops::set_ambient_intensity(&mut scene.get(lua)?.borrow_mut(), v);
            Ok(())
        }),
    )
}

/// `{Get,Set}AmbientMode` (`"Color"`, `"Gradient"` or `"Skybox"`) and
/// `{Get,Set}Ambient{Sky,Equator,Ground}Color` (`r, g, b`): Unity's
/// `RenderSettings.ambientMode` and its trilight colours.
fn register_ambient_gradient(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    use rusty_scene::{AmbientMode, AmbientSettings};
    put(
        table,
        "GetAmbientMode",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().ambient.mode.name())),
    )?;
    put(
        table,
        "SetAmbientMode",
        lua.create_function(move |lua, name: String| { let scene = &*scene.get(lua)?;
            let mode = AmbientMode::from_name(&name).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "Lighting.SetAmbientMode: unknown mode {name:?} (\"Color\", \"Gradient\" or \"Skybox\")"
                ))
            })?;
            env_ops::set_ambient_mode(&mut scene.borrow_mut().ambient, mode);
            Ok(())
        }),
    )?;
    type Read = fn(&AmbientSettings) -> Vec3;
    type Write = fn(&mut AmbientSettings, Vec3);
    let colors: [(&str, Read, Write); 3] = [
        ("Sky", |a| a.sky, env_ops::set_ambient_sky_color),
        ("Equator", |a| a.equator, env_ops::set_ambient_equator_color),
        ("Ground", |a| a.ground, env_ops::set_ambient_ground_color),
    ];
    for (name, read, write) in colors {
        put(
            table,
            &format!("GetAmbient{name}Color"),
            lua.create_function(move |lua, ()| {
                let c = read(&scene.get(lua)?.borrow().ambient);
                Ok((c.x, c.y, c.z))
            }),
        )?;
        put(
            table,
            &format!("SetAmbient{name}Color"),
            lua.create_function(move |lua, (r, g, b): (f32, f32, f32)| {
                write(
                    &mut scene.get(lua)?.borrow_mut().ambient,
                    Vec3::new(r, g, b),
                );
                Ok(())
            }),
        )?;
    }
    Ok(())
}

type SkyColorOp = fn(&mut rusty_scene::SkySettings, Option<Vec3>);

fn register_sky_colors(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    type Read = fn(&rusty_scene::SkySettings) -> Option<Vec3>;
    let colors: [(&str, Read, SkyColorOp); 3] = [
        ("Zenith", |s| s.zenith, env_ops::set_sky_zenith),
        ("Horizon", |s| s.horizon, env_ops::set_sky_horizon),
        ("Ground", |s| s.ground, env_ops::set_sky_ground),
    ];
    for (name, read, write) in colors {
        put(
            table,
            &format!("GetSky{name}Color"),
            lua.create_function(move |lua, ()| {
                let c = read(&scene.get(lua)?.borrow().sky);
                Ok((c.map(|c| c.x), c.map(|c| c.y), c.map(|c| c.z)))
            }),
        )?;
        put(
            table,
            &format!("SetSky{name}Color"),
            lua.create_function(move |lua, rgb: (Option<f32>, Option<f32>, Option<f32>)| {
                let color = match rgb {
                    (Some(r), Some(g), Some(b)) => Some(Vec3::new(r, g, b)),
                    (None, None, None) => None,
                    _ => {
                        return Err(mlua::Error::runtime(format!(
                            "Lighting.SetSky{name}Color: give r, g, b, or nothing to follow"
                        )))
                    }
                };
                write(&mut scene.get(lua)?.borrow_mut().sky, color);
                Ok(())
            }),
        )?;
    }
    Ok(())
}

fn register_clouds(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    type Read = fn(&rusty_scene::SkySettings) -> f32;
    type Write = fn(&mut rusty_scene::SkySettings, f32);
    let knobs: [(&str, Read, Write); 2] = [
        (
            "Coverage",
            |s| s.cloud_coverage,
            env_ops::set_cloud_coverage,
        ),
        ("Opacity", |s| s.cloud_opacity, env_ops::set_cloud_opacity),
    ];
    for (name, read, write) in knobs {
        put(
            table,
            &format!("GetCloud{name}"),
            lua.create_function(move |lua, ()| Ok(read(&scene.get(lua)?.borrow().sky))),
        )?;
        put(
            table,
            &format!("SetCloud{name}"),
            lua.create_function(move |lua, v: f32| {
                write(&mut scene.get(lua)?.borrow_mut().sky, v);
                Ok(())
            }),
        )?;
    }
    Ok(())
}
