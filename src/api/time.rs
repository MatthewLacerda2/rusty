//! src/api/time.rs — `Time` namespace.
//!
//! `deltaTime` (scaled) / `unscaledDeltaTime` (raw) / `fixedDeltaTime` /
//! `frameCount` accessors plus `SetTimeScale` / `GetTimeScale`, backed by the
//! `time::Time` resource the `GameWorld` advances once per tick.
//!
//! Pause / step / resume (issue #283) live here too: `Pause()` / `Resume()` /
//! `Step(n)` set the loop-level control fields on the same `Time` resource that the
//! windowed frame loop reads each frame. `Pause` is a hard loop halt distinct from
//! `SetTimeScale(0)` — see the field docs on `time::Time` and `docs/scripting-api.md`.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::time::Time;

/// Register the `Time` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    time: &'scope RefCell<Time>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_clock(scope, &table, time)?;
    register_pause_step(scope, &table, time)?;
    lua.globals().set("Time", table).map_err(|e| e.to_string())
}

/// The read-only clock accessors plus the `time_scale` get/set pair.
fn register_clock<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    time: &'scope RefCell<Time>,
) -> Reg {
    put(
        table,
        "deltaTime",
        scope.create_function(|_, ()| Ok(time.borrow().delta_time)),
    )?;
    put(
        table,
        "unscaledDeltaTime",
        scope.create_function(|_, ()| Ok(time.borrow().unscaled_delta_time)),
    )?;
    put(
        table,
        "fixedDeltaTime",
        scope.create_function(|_, ()| Ok(time.borrow().fixed_delta_time)),
    )?;
    put(
        table,
        "frameCount",
        scope.create_function(|_, ()| Ok(time.borrow().frame_count)),
    )?;
    put(
        table,
        "SetTimeScale",
        scope.create_function(|_, scale: f32| {
            time.borrow_mut().set_time_scale(scale);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTimeScale",
        scope.create_function(|_, ()| Ok(time.borrow().time_scale)),
    )
}

/// Pause / step / resume (issue #283): the loop-level control verbs the windowed
/// frame loop reads each frame, plus `IsPaused` / `PendingSteps` readers so the
/// control state is observable through the same surface.
fn register_pause_step<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    time: &'scope RefCell<Time>,
) -> Reg {
    put(
        table,
        "Pause",
        scope.create_function(|_, ()| {
            time.borrow_mut().pause();
            Ok(())
        }),
    )?;
    put(
        table,
        "Resume",
        scope.create_function(|_, ()| {
            time.borrow_mut().resume();
            Ok(())
        }),
    )?;
    put(
        table,
        "Step",
        scope.create_function(|_, n: u32| {
            // Enqueues N fixed-dt steps **only while paused** (no-op otherwise — see
            // `Time::request_steps`). The windowed loop drains them at FIXED_DELTA_TIME.
            time.borrow_mut().request_steps(n);
            Ok(())
        }),
    )?;
    put(
        table,
        "IsPaused",
        scope.create_function(|_, ()| Ok(time.borrow().paused)),
    )?;
    put(
        table,
        "PendingSteps",
        scope.create_function(|_, ()| Ok(time.borrow().pending_steps)),
    )
}
