//! src/api/light_probe_proxy_volume.rs — `LightProbeProxyVolume` namespace (#1672).
//!
//! Get/Set over an entity's `LightProbeProxyVolumeComponent` (Unity's
//! `LightProbeProxyVolume`): the bounding box mode and custom box, the resolution
//! mode with its density or per-axis counts, and the quality. Every setter routes
//! through the shared `scene::authoring::rendering::light_probe_proxy_volume` ops
//! the inspector card uses. Without a volume a getter returns `nil` (zeros for a
//! vector or a resolution), and a setter is an error naming the entity. Attach one with
//! `Scene.AddComponent(id, "LightProbeProxyVolume")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::components::{
    LightProbeProxyVolumeComponent as Volume, ProxyBoundsMode, ProxyQuality, ProxyResolutionMode,
};
use crate::scene::authoring::rendering::light_probe_proxy_volume as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;

const NS: &str = "LightProbeProxyVolume";

/// Register the `LightProbeProxyVolume` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_modes(scope, &t, scene)?;
    register_box(scope, &t, scene)?;
    register_resolution(scope, &t, scene)?;
    lua.globals().set(NS, t).map_err(|e| e.to_string())
}

fn err(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}

/// `id`'s volume, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<Volume> {
    scene
        .borrow()
        .world
        .light_probe_proxy_volume(id)
        .map(|v| v.clone())
}

/// Apply a shared op to `id`'s volume; an error without one.
fn set(scene: SceneCell, id: u32, op: impl FnOnce(&mut Volume)) -> mlua::Result<()> {
    let mut s = scene.borrow_mut();
    let mut v = s
        .world
        .light_probe_proxy_volume_mut(id)
        .ok_or_else(|| err(format!("entity {id} has no {NS}")))?;
    op(&mut v);
    Ok(())
}

/// `name` parsed by `parse`, or an error listing `names`.
fn parsed<T>(
    what: &str,
    name: &str,
    parse: fn(&str) -> Option<T>,
    names: String,
) -> mlua::Result<T> {
    parse(name).ok_or_else(|| err(format!("unknown {what} '{name}' ({names})")))
}

/// `Get/SetBoundingBoxMode`, `Get/SetResolutionMode`, `Get/SetQuality`, by name.
fn register_modes<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).map(|v| v.bounds_mode.name()))
    });
    put(t, "GetBoundingBoxMode", f)?;
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let id = id.live(NS, "SetBoundingBoxMode")?;
        let names = ProxyBoundsMode::names();
        let mode = parsed("bounding box mode", &name, ProxyBoundsMode::parse, names)?;
        set(scene, id, |v| ops::set_bounds_mode(v, mode))
    });
    put(t, "SetBoundingBoxMode", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).map(|v| v.resolution_mode.name()))
    });
    put(t, "GetResolutionMode", f)?;
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let id = id.live(NS, "SetResolutionMode")?;
        let names = ProxyResolutionMode::names();
        let mode = parsed("resolution mode", &name, ProxyResolutionMode::parse, names)?;
        set(scene, id, |v| ops::set_resolution_mode(v, mode))
    });
    put(t, "SetResolutionMode", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).map(|v| v.quality.name()))
    });
    put(t, "GetQuality", f)?;
    let f = scope.create_function(move |_, (id, name): (EntityArg, String)| {
        let id = id.live(NS, "SetQuality")?;
        let quality = parsed("quality", &name, ProxyQuality::parse, ProxyQuality::names())?;
        set(scene, id, |v| ops::set_quality(v, quality))
    });
    put(t, "SetQuality", f)
}

/// `Get/SetSize` and `Get/SetOrigin` (the custom box), as `x, y, z`.
fn register_box<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    type Field = (fn(&Volume) -> Vec3, fn(&mut Volume, Vec3));
    let fields: [(&str, Field); 2] = [
        ("Size", (|v| v.size, ops::set_size)),
        ("Origin", (|v| v.origin, ops::set_origin)),
    ];
    for (name, (read, write)) in fields {
        let f = scope.create_function(move |_, id: EntityArg| {
            let v = get(scene, id.read()).map_or(Vec3::ZERO, |v| read(&v));
            Ok((v.x, v.y, v.z))
        });
        put(t, &format!("Get{name}"), f)?;
        let verb = format!("Set{name}");
        let f = scope.create_function(move |_, (id, x, y, z): (EntityArg, f32, f32, f32)| {
            let id = id.live(NS, &verb)?;
            set(scene, id, |v| write(v, Vec3::new(x, y, z)))
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}

/// `Get/SetProbeDensity` and `Get/SetGridResolution` (`x, y, z`).
fn register_resolution<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: EntityArg| {
        Ok(get(scene, id.read()).map(|v| v.probe_density))
    });
    put(t, "GetProbeDensity", f)?;
    let f = scope.create_function(move |_, (id, d): (EntityArg, f32)| {
        let id = id.live(NS, "SetProbeDensity")?;
        set(scene, id, |v| ops::set_probe_density(v, d))
    });
    put(t, "SetProbeDensity", f)?;
    let f = scope.create_function(move |_, id: EntityArg| {
        let [x, y, z] = get(scene, id.read()).map_or([0; 3], |v| v.grid_resolution);
        Ok((x, y, z))
    });
    put(t, "GetGridResolution", f)?;
    let f = scope.create_function(move |_, (id, x, y, z): (EntityArg, u32, u32, u32)| {
        let id = id.live(NS, "SetGridResolution")?;
        set(scene, id, |v| ops::set_grid_resolution(v, [x, y, z]))
    });
    put(t, "SetGridResolution", f)
}

#[cfg(test)]
#[path = "light_probe_proxy_volume_tests.rs"]
mod tests;
