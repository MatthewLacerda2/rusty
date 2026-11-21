use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::ribbon_style::{self, Ribbon};
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::LineComponent;
use rusty_edit::authoring::ribbon::line as ops;
use rusty_scene::Scene;

type IdPoint = (EntityArg, usize, f32, f32, f32);

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_positions(lua, &t, scene)?;
    register_flags(lua, &t, scene)?;
    ribbon_style::register(lua, &t, scene, Ribbon::Line)?;
    lua.globals().set("Line", t).map_err(|e| e.to_string())
}

fn read<T>(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&LineComponent) -> T) -> Option<T> {
    scene.borrow().world.line(id).map(|l| f(&l))
}

fn write(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut LineComponent)) {
    if let Some(mut l) = scene.borrow_mut().world.line_mut(id) {
        f(&mut l);
    }
}

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
