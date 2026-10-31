//! src/api/lighting.rs — `Lighting` namespace: the scene's environment and the
//! one-button bakes.
//!
//! The environment (#1612) is Unity's `RenderSettings`: the skybox panorama and the
//! ambient light, read and written through the same `authoring::environment` ops the
//! Scene Settings panel calls, saved with the scene and present in every build.
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
