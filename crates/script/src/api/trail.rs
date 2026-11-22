use std::cell::RefCell;

use mlua::{Lua, Table};

use super::ribbon_style::{self, Ribbon};
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::TrailComponent;
use rusty_edit::authoring::ribbon::trail as ops;
use rusty_scene::Scene;

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_settings(lua, &t, scene)?;
    register_points(lua, &t, scene)?;
    ribbon_style::register(lua, &t, scene, Ribbon::Trail)?;
    lua.globals().set("Trail", t).map_err(|e| e.to_string())
}

fn read<T>(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&TrailComponent) -> T) -> Option<T> {
    scene.borrow().world.trail(id).map(|t| f(&t))
}

fn write(scene: &RefCell<Scene>, id: u32, f: impl FnOnce(&mut TrailComponent)) {
    if let Some(mut t) = scene.borrow_mut().world.trail_mut(id) {
        f(&mut t);
    }
}

fn register_settings(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(read(&*scene.get(lua)?, id.read(), |t| t.emitting) == Some(true))
    });
    put(t, "IsEmitting", f)?;
    let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
        let id = id.live("Trail", "SetEmitting")?;
        write(&*scene.get(lua)?, id, |t| ops::set_emitting(t, on));
        Ok(())
    });
    put(t, "SetEmitting", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(read(&*scene.get(lua)?, id.read(), |t| t.time).unwrap_or(0.0))
    });
    put(t, "GetTime", f)?;
    let f = lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
        let id = id.live("Trail", "SetTime")?;
        write(&*scene.get(lua)?, id, |t| ops::set_time(t, v));
        Ok(())
    });
    put(t, "SetTime", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(read(&*scene.get(lua)?, id.read(), |t| t.min_vertex_distance).unwrap_or(0.0))
    });
    put(t, "GetMinVertexDistance", f)?;
    let f = lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
        let id = id.live("Trail", "SetMinVertexDistance")?;
        write(&*scene.get(lua)?, id, |t| {
            ops::set_min_vertex_distance(t, v)
        });
        Ok(())
    });
    put(t, "SetMinVertexDistance", f)
}

fn register_points(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let id = id.live("Trail", "Clear")?;
        write(&*scene.get(lua)?, id, TrailComponent::clear);
        Ok(())
    });
    put(t, "Clear", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(read(&*scene.get(lua)?, id.read(), |t| t.runtime.points.len()).unwrap_or(0))
    });
    put(t, "GetPositionCount", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let points = read(&*scene.get(lua)?, id.read(), |t| {
            t.positions().map(|p| p.to_array()).collect()
        });
        Ok(points.unwrap_or_else(Vec::new))
    });
    put(t, "GetPositions", f)
}
