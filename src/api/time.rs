//! src/api/time.rs — `Time` namespace.
//!
//! `deltaTime` (scaled) / `unscaledDeltaTime` (raw) / `fixedDeltaTime` /
//! `frameCount` / `time` / `unscaledTime` accessors plus `SetTimeScale` / `GetTimeScale`, backed by the
//! `time::Time` resource the `GameWorld` advances once per tick.
//!
//! Pause / step / resume (issue #283) live here too: `Pause()` / `Resume()` /
//! `Step(n)` set the loop-level control fields on the same `Time` resource that the
//! windowed frame loop reads each frame. `Pause` is a hard loop halt distinct from
//! `SetTimeScale(0)` — see the field docs on `time::Time` and `docs/api/Time.md`.

use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use crate::time::Time;

/// Register the `Time` namespace onto `lua`.
pub fn register(lua: &Lua, time: ApiCell<Time>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_clock(lua, &table, time)?;
    register_pause_step(lua, &table, time)?;
    lua.globals().set("Time", table).map_err(|e| e.to_string())
}

/// The read-only clock accessors plus the `time_scale` get/set pair.
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
    // Scaled game time (#398) — the same clock shaders read as `camera.time`.
    put(
        table,
        "time",
        lua.create_function(move |lua, ()| Ok(time.get(lua)?.borrow().time)),
    )?;
    // Unscaled time (#427) — the clock custom UI shaders read.
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

/// Pause / step / resume (issue #283): the loop-level control verbs the windowed
/// frame loop reads each frame, plus `IsPaused` / `PendingSteps` readers so the
/// control state is observable through the same surface.
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
            // Enqueues N fixed-dt steps **only while paused** (no-op otherwise — see
            // `Time::request_steps`). The windowed loop drains them at FIXED_DELTA_TIME.
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
