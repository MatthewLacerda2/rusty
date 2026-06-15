//! src/api/time.rs — `Time` namespace.
//!
//! `deltaTime` (scaled) / `unscaledDeltaTime` (raw) / `fixedDeltaTime` /
//! `frameCount` accessors plus `SetTimeScale` / `GetTimeScale`, backed by the
//! `time::Time` resource the `GameWorld` advances once per tick. Scoped callbacks
//! over a borrowed `&RefCell<&mut Time>` (#57).

use std::cell::RefCell;

use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::time::Time;

/// Register the `Time` namespace onto `lua`.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    time: &'scope RefCell<&'scope mut Time>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "deltaTime",
        scope.create_function(move |_, ()| Ok(time.borrow().delta_time)),
    )?;

    put(
        &table,
        "unscaledDeltaTime",
        scope.create_function(move |_, ()| Ok(time.borrow().unscaled_delta_time)),
    )?;

    put(
        &table,
        "fixedDeltaTime",
        scope.create_function(move |_, ()| Ok(time.borrow().fixed_delta_time)),
    )?;

    put(
        &table,
        "frameCount",
        scope.create_function(move |_, ()| Ok(time.borrow().frame_count)),
    )?;

    put(
        &table,
        "SetTimeScale",
        scope.create_function(move |_, scale: f32| {
            time.borrow_mut().set_time_scale(scale);
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetTimeScale",
        scope.create_function(move |_, ()| Ok(time.borrow().time_scale)),
    )?;

    lua.globals().set("Time", table).map_err(|e| e.to_string())
}
