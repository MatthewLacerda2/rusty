//! src/api/trail.rs — `Trail` namespace (#441).
//!
//! Script control over an entity's `TrailComponent` (Unity's `TrailRenderer`):
//! start/stop recording, wipe it (`Clear`, e.g. after a teleport), tune how long
//! points live and how far apart they are, read the recorded points back, and
//! restyle it with the width / colour / texture / blend verbs it shares with
//! `Line` (`ribbon_style`). Setters route through `scene::authoring::trail` /
//! `ribbon`. Without a Trail, getters return a neutral default; setters no-op.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::ribbon_style::{self, Ribbon};
use super::{put, Reg};
use crate::components::TrailComponent;
use crate::scene::authoring::trail as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;

/// Register the `Trail` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_settings(scope, &t, scene)?;
    register_points(scope, &t, scene)?;
    ribbon_style::register(scope, &t, scene, Ribbon::Trail)?;
    lua.globals().set("Trail", t).map_err(|e| e.to_string())
}

/// Read `id`'s Trail through `f`, or `None` without one.
fn read<T>(scene: SceneCell, id: u32, f: impl FnOnce(&TrailComponent) -> T) -> Option<T> {
    scene.borrow().world.trail(id).map(|t| f(&t))
}

/// Apply `f` to `id`'s Trail, when it has one.
fn write(scene: SceneCell, id: u32, f: impl FnOnce(&mut TrailComponent)) {
    if let Some(mut t) = scene.borrow_mut().world.trail_mut(id) {
        f(&mut t);
    }
}

/// `IsEmitting` / `SetEmitting`, `GetTime` / `SetTime`,
/// `GetMinVertexDistance` / `SetMinVertexDistance`.
fn register_settings<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    let f =
        scope.create_function(move |_, id: u32| Ok(read(scene, id, |t| t.emitting) == Some(true)));
    put(t, "IsEmitting", f)?;
    let f = scope.create_function(move |_, (id, on): (u32, bool)| {
        write(scene, id, |t| ops::set_emitting(t, on));
        Ok(())
    });
    put(t, "SetEmitting", f)?;
    let f = scope.create_function(move |_, id: u32| Ok(read(scene, id, |t| t.time).unwrap_or(0.0)));
    put(t, "GetTime", f)?;
    let f = scope.create_function(move |_, (id, v): (u32, f32)| {
        write(scene, id, |t| ops::set_time(t, v));
        Ok(())
    });
    put(t, "SetTime", f)?;
    let f = scope.create_function(move |_, id: u32| {
        Ok(read(scene, id, |t| t.min_vertex_distance).unwrap_or(0.0))
    });
    put(t, "GetMinVertexDistance", f)?;
    let f = scope.create_function(move |_, (id, v): (u32, f32)| {
        write(scene, id, |t| ops::set_min_vertex_distance(t, v));
        Ok(())
    });
    put(t, "SetMinVertexDistance", f)
}

/// `Clear`, `GetPositionCount`, `GetPositions` (a list of `{x, y, z}`, oldest
/// first, world space).
fn register_points<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| {
        write(scene, id, TrailComponent::clear);
        Ok(())
    });
    put(t, "Clear", f)?;
    let f = scope.create_function(move |_, id: u32| {
        Ok(read(scene, id, |t| t.runtime.points.len()).unwrap_or(0))
    });
    put(t, "GetPositionCount", f)?;
    let f = scope.create_function(move |_, id: u32| {
        let points = read(scene, id, |t| t.positions().map(|p| p.to_array()).collect());
        Ok(points.unwrap_or_else(Vec::new))
    });
    put(t, "GetPositions", f)
}
