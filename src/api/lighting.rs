//! src/api/lighting.rs — `Lighting` namespace: the scene's environment and the
//! one-button bakes.
//!
//! The environment (#1612) is Unity's `RenderSettings`: the skybox panorama, the
//! ambient light (its colour and intensity, and #1647's source: `Color`,
//! `Gradient` with sky, equator and ground colours, or #1675's `Skybox`) and the
//! procedural sky's look (#1620: zenith, horizon and ground colours, cloud coverage
//! and opacity), read and written through the same
//! `authoring::environment` ops the Scene Settings panel calls, saved with the scene
//! (a change made in Play reverts at Stop, like every scene value) and present in
//! every build.
//!
//! `Lighting.Bake()` is the "place well + bake right now" workflow (#246): in one call
//! it auto-places light probes and reflection probes from the static scene (unless a set
//! is already manually authored), then runs the existing multi-bounce light-probe bake
//! (#250, #285) and GGX reflection-probe bake (#252). It is the orchestration over the `Probe`
//! and `Reflection` namespaces — manual placement on those stays available; this is the
//! batteries-included path. `Lighting.BakeLightmaps()` / `ClearLightmaps()` (#438) bake
//! static geometry's lightmaps beside it, and `Lighting.Generate()` (#832) runs both
//! in order and writes the result, also from `dev`.
//!
//! The bakes are dev-only (they drive a headless GPU and write authoring artifacts),
//! so `dev` installs them onto this table through [`super::extend`] (#737), beside
//! the editor's Generate Lighting button that runs the same orchestration. In a ship
//! build the `Lighting` table holds only the environment, matching the stripped
//! agentic layer.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::scene::authoring::environment as env_ops;
use crate::scene::Scene;

/// Register the `Lighting` namespace table, with the environment's getters and
/// setters, onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_skybox(scope, &table, scene)?;
    register_ambient(scope, &table, scene)?;
    register_ambient_gradient(scope, &table, scene)?;
    register_sky_colors(scope, &table, scene)?;
    register_clouds(scope, &table, scene)?;
    lua.globals()
        .set("Lighting", table)
        .map_err(|e| e.to_string())
}

/// `{Get,Set}Skybox`: the panorama's path, `""` for the procedural sky.
fn register_skybox<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetSkybox",
        scope.create_function(|_, ()| Ok(scene.borrow().skybox_path.clone())),
    )?;
    put(
        table,
        "SetSkybox",
        scope.create_function(|_, path: Option<String>| {
            env_ops::set_skybox(&mut scene.borrow_mut(), path.as_deref().unwrap_or(""));
            Ok(())
        }),
    )
}

/// `{Get,Set}AmbientColor` (`r, g, b`) and `{Get,Set}AmbientIntensity`.
fn register_ambient<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetAmbientColor",
        scope.create_function(|_, ()| {
            let c = scene.borrow().ambient_color;
            Ok((c.x, c.y, c.z))
        }),
    )?;
    put(
        table,
        "SetAmbientColor",
        scope.create_function(|_, (r, g, b): (f32, f32, f32)| {
            env_ops::set_ambient_color(&mut scene.borrow_mut(), Vec3::new(r, g, b));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetAmbientIntensity",
        scope.create_function(|_, ()| Ok(scene.borrow().ambient_intensity)),
    )?;
    put(
        table,
        "SetAmbientIntensity",
        scope.create_function(|_, v: f32| {
            env_ops::set_ambient_intensity(&mut scene.borrow_mut(), v);
            Ok(())
        }),
    )
}

/// `{Get,Set}AmbientMode` (`"Color"`, `"Gradient"` or `"Skybox"`) and
/// `{Get,Set}Ambient{Sky,Equator,Ground}Color` (`r, g, b`): Unity's
/// `RenderSettings.ambientMode` and its trilight colours.
fn register_ambient_gradient<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    use crate::scene::{AmbientMode, AmbientSettings};
    put(
        table,
        "GetAmbientMode",
        scope.create_function(|_, ()| Ok(scene.borrow().ambient.mode.name())),
    )?;
    put(
        table,
        "SetAmbientMode",
        scope.create_function(|_, name: String| {
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
            scope.create_function(move |_, ()| {
                let c = read(&scene.borrow().ambient);
                Ok((c.x, c.y, c.z))
            }),
        )?;
        put(
            table,
            &format!("SetAmbient{name}Color"),
            scope.create_function(move |_, (r, g, b): (f32, f32, f32)| {
                write(&mut scene.borrow_mut().ambient, Vec3::new(r, g, b));
                Ok(())
            }),
        )?;
    }
    Ok(())
}

/// A sky colour setter: `(r, g, b)` sets it, no arguments (or `nil`) goes back to
/// following the ambient colour.
type SkyColorOp = fn(&mut crate::scene::SkySettings, Option<Vec3>);

/// `{Get,Set}Sky{Zenith,Horizon,Ground}Color`: `r, g, b`, or `nil` while the colour
/// follows the ambient one.
fn register_sky_colors<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    type Read = fn(&crate::scene::SkySettings) -> Option<Vec3>;
    let colors: [(&str, Read, SkyColorOp); 3] = [
        ("Zenith", |s| s.zenith, env_ops::set_sky_zenith),
        ("Horizon", |s| s.horizon, env_ops::set_sky_horizon),
        ("Ground", |s| s.ground, env_ops::set_sky_ground),
    ];
    for (name, read, write) in colors {
        put(
            table,
            &format!("GetSky{name}Color"),
            scope.create_function(move |_, ()| {
                let c = read(&scene.borrow().sky);
                Ok((c.map(|c| c.x), c.map(|c| c.y), c.map(|c| c.z)))
            }),
        )?;
        put(
            table,
            &format!("SetSky{name}Color"),
            scope.create_function(move |_, rgb: (Option<f32>, Option<f32>, Option<f32>)| {
                let color = match rgb {
                    (Some(r), Some(g), Some(b)) => Some(Vec3::new(r, g, b)),
                    (None, None, None) => None,
                    _ => {
                        return Err(mlua::Error::runtime(format!(
                            "Lighting.SetSky{name}Color: give r, g, b, or nothing to follow"
                        )))
                    }
                };
                write(&mut scene.borrow_mut().sky, color);
                Ok(())
            }),
        )?;
    }
    Ok(())
}

/// `{Get,Set}CloudCoverage` and `{Get,Set}CloudOpacity`, each `0..1`.
fn register_clouds<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    type Read = fn(&crate::scene::SkySettings) -> f32;
    type Write = fn(&mut crate::scene::SkySettings, f32);
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
            scope.create_function(move |_, ()| Ok(read(&scene.borrow().sky))),
        )?;
        put(
            table,
            &format!("SetCloud{name}"),
            scope.create_function(move |_, v: f32| {
                write(&mut scene.borrow_mut().sky, v);
                Ok(())
            }),
        )?;
    }
    Ok(())
}
