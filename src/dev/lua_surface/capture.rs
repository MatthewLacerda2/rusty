//! src/dev/lua_surface/capture.rs — `Audio.StartCapture` / `Audio.StopCapture`
//! (#1579): record the final mix the speakers get to a WAV, so an agent that cannot
//! hear can measure it (`Sound.Level`).
//!
//! A measuring tool, not a game feature, so it is added here, onto the `Audio` table
//! `api` registered, and a ship build has neither verb (#737). In the windowed
//! editor it taps the real device; with no device (a scenario, a silent box) the
//! maestro mixes offline in step with the engine's time (`audio::capture`).

use std::path::Path;

use mlua::Lua;

use crate::api::{global_table, put, ApiScopedCtx, Reg};

/// Add `StartCapture(path)` and `StopCapture()` to the `Audio` table. Both raise a
/// Lua error on a refusal (a second start, a stop with nothing running, a file that
/// cannot be written); `StopCapture` returns the path it wrote.
pub(super) fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let table = global_table(lua, "Audio")?;
    let audio = ctx.audio;
    put(
        &table,
        "StartCapture",
        scope.create_function(move |_, path: String| {
            let started = audio.borrow_mut().start_capture(Path::new(&path));
            started.map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        &table,
        "StopCapture",
        scope.create_function(move |_, ()| {
            let path = audio.borrow_mut().stop_capture();
            let path = path.map_err(mlua::Error::RuntimeError)?;
            Ok(path.display().to_string())
        }),
    )
}
