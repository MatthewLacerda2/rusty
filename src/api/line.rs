//! src/api/line.rs — `Line` namespace (#441).
//!
//! Script control over an entity's `LineComponent` (Unity's `LineRenderer`): set
//! its points all at once (`SetPositions`, e.g. a grenade arc each frame) or one
//! at a time (0-based, like `Probe` and `Reflection`), choose world or local space
//! and looping, and restyle it with the verbs it shares with `Trail`
//! (`ribbon_style`). Setters route through `scene::authoring::ribbon::line` / `ribbon`.
//! Without a Line, getters return a neutral default; setters no-op.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::ribbon_style::{self, Ribbon};
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::LineComponent;
use crate::scene::authoring::ribbon::line as ops;
use crate::scene::Scene;

/// `(id, index, x, y, z)`.
type IdPoint = (EntityArg, usize, f32, f32, f32);

/// Register the `Line` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_positions(lua, &t, scene)?;
    register_flags(lua, &t, scene)?;
    ribbon_style::register(lua, &t, scene, Ribbon::Line)?;
    lua.globals().set("Line", t).map_err(|e| e.to_string())
}

/// Read `id`'s Line through `f`, or `None` without one.
fn read<T>(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&LineComponent) -> T) -> Option<T> {
    scene.borrow().world.line(id).map(|l| f(&l))
}

/// Apply `f` to `id`'s Line, when it has one.
fn write(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut LineComponent)) {
    if let Some(mut l) = scene.borrow_mut().world.line_mut(id) {
        f(&mut l);
    }
}

/// `GetPositions` / `SetPositions` (a list of `{x, y, z}`), `GetPosition` /
/// `SetPosition` (one 0-based point), `GetPositionCount` / `SetPositionCount`.
fn register_positions(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let points = read(&*scene.get(lua)?, id.read(), |l| {
            l.positions.iter().map(|p| p.to_array()).collect()
        });
        Ok(points.unwrap_or_else(Vec::new))
    });
    put(t, "GetPositions", f)?;
    let f = lua.create_function(move |lua, (id, points): (EntityArg, Vec<[f32; 3]>)| {
        let id = id.live("Line", "SetPositions")?;
        let points: Vec<Vec3> = points.into_iter().map(Vec3::from).collect();
        write(&*scene.get(lua)?, id, |l| ops::set_positions(l, &points));
        Ok(())
    });
    put(t, "SetPositions", f)?;
    let f = lua.create_function(move |lua, (id, i): (EntityArg, usize)| {
        let p = read(&*scene.get(lua)?, id.read(), |l| {
            l.positions.get(i).copied()
        })
        .flatten();
        let p = p.unwrap_or(Vec3::ZERO);
        Ok((p.x, p.y, p.z))
    });
    put(t, "GetPosition", f)?;
    let f = lua.create_function(move |lua, (id, i, x, y, z): IdPoint| {
        let id = id.live("Line", "SetPosition")?;
        write(&*scene.get(lua)?, id, |l| {
            ops::set_position(l, i, Vec3::new(x, y, z))
        });
        Ok(())
    });
    put(t, "SetPosition", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(read(&*scene.get(lua)?, id.read(), |l| l.positions.len()).unwrap_or(0))
    });
    put(t, "GetPositionCount", f)?;
    let f = lua.create_function(move |lua, (id, n): (EntityArg, usize)| {
        let id = id.live("Line", "SetPositionCount")?;
        write(&*scene.get(lua)?, id, |l| ops::set_position_count(l, n));
        Ok(())
    });
    put(t, "SetPositionCount", f)
}

/// `GetUseWorldSpace` / `SetUseWorldSpace`, `GetLoop` / `SetLoop`.
fn register_flags(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(read(&*scene.get(lua)?, id.read(), |l| l.use_world_space) == Some(true))
    });
    put(t, "GetUseWorldSpace", f)?;
    let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
        let id = id.live("Line", "SetUseWorldSpace")?;
        write(&*scene.get(lua)?, id, |l| ops::set_use_world_space(l, on));
        Ok(())
    });
    put(t, "SetUseWorldSpace", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(read(&*scene.get(lua)?, id.read(), |l| l.looping) == Some(true))
    });
    put(t, "GetLoop", f)?;
    let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
        let id = id.live("Line", "SetLoop")?;
        write(&*scene.get(lua)?, id, |l| ops::set_looping(l, on));
        Ok(())
    });
    put(t, "SetLoop", f)
}
