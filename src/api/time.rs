//! src/api/time.rs — `Time` namespace.
//!
//! `deltaTime` (scaled) / `unscaledDeltaTime` (raw) / `fixedDeltaTime` /
//! `frameCount` accessors plus `SetTimeScale` / `GetTimeScale`, backed by the
//! `time::Time` resource the `GameWorld` advances once per tick.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::time::Time;

/// Register the `Time` namespace onto `lua`.
pub fn register(lua: &Lua, time: &Rc<RefCell<Time>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let t = Rc::clone(time);
    put(
        &table,
        "deltaTime",
        lua.create_function(move |_, ()| Ok(t.borrow().delta_time)),
    )?;

    let t = Rc::clone(time);
    put(
        &table,
        "unscaledDeltaTime",
        lua.create_function(move |_, ()| Ok(t.borrow().unscaled_delta_time)),
    )?;

    let t = Rc::clone(time);
    put(
        &table,
        "fixedDeltaTime",
        lua.create_function(move |_, ()| Ok(t.borrow().fixed_delta_time)),
    )?;

    let t = Rc::clone(time);
    put(
        &table,
        "frameCount",
        lua.create_function(move |_, ()| Ok(t.borrow().frame_count)),
    )?;

    let t = Rc::clone(time);
    put(
        &table,
        "SetTimeScale",
        lua.create_function(move |_, scale: f32| {
            t.borrow_mut().set_time_scale(scale);
            Ok(())
        }),
    )?;

    let t = Rc::clone(time);
    put(
        &table,
        "GetTimeScale",
        lua.create_function(move |_, ()| Ok(t.borrow().time_scale)),
    )?;

    lua.globals().set("Time", table).map_err(|e| e.to_string())
}
