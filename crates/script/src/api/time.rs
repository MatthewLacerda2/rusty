//! `Pause` halts the loop entirely, unlike `SetTimeScale(0)`.

use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_time::Time;

pub fn register(lua: &Lua, time: ApiCell<Time>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_clock(lua, &table, time)?;
    register_pause_step(lua, &table, time)?;
    lua.globals().set("Time", table).map_err(|e| e.to_string())
}

fn register_clock(lua: &mlua::Lua, table: &mlua::Table, time: ApiCell<Time>) -> Reg {
    put(
        table,
        "deltaTime",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().delta_time)),
    )?;
    put(
        table,
        "unscaledDeltaTime",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().unscaled_delta_time)),
    )?;
    put(
        table,
        "fixedDeltaTime",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().fixed_delta_time)),
    )?;
    put(
        table,
        "frameCount",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().frame_count)),
    )?;
    put(
        table,
        "time",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().time)),
    )?;
    put(
        table,
        "unscaledTime",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().unscaled_time)),
    )?;
    put(
        table,
        "SetTimeScale",
        lua.create_function(move |lua, scale: f32| {
            time.get(lua)?.borrow_mut().set_time_scale(scale);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTimeScale",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().time_scale)),
    )
}

fn register_pause_step(lua: &mlua::Lua, table: &mlua::Table, time: ApiCell<Time>) -> Reg {
    put(
        table,
        "Pause",
        lua.create_function(move |lua, ()| {
            time.get(lua)?.borrow_mut().pause();
            Ok(())
        }),
    )?;
    put(
        table,
        "Resume",
        lua.create_function(move |lua, ()| {
            time.get(lua)?.borrow_mut().resume();
            Ok(())
        }),
    )?;
    put(
        table,
        "Step",
        lua.create_function(move |lua, n: u32| {
            time.get(lua)?.borrow_mut().request_steps(n);
            Ok(())
        }),
    )?;
    put(
        table,
        "IsPaused",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().paused)),
    )?;
    put(
        table,
        "PendingSteps",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().pending_steps)),
    )
}
