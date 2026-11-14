//! `Harness.Warnings([after])` (#2054): the run's console warnings and errors, so a
//! scenario can assert a clean run (`Harness.Expect(#Harness.Warnings() == 0, …)`).
//!
//! It reads the harness console, the one `console.log` is written from, which keeps
//! every warning and error of the run past the buffer's cap
//! (`ConsoleLogs::keep_warnings`). Engine warnings join it after each tick, and
//! again here, so one raised outside a tick (loading the scene) is in too.

use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table};

use super::Shared;
use rusty_script::scripting::LogLevel;

/// Register `Harness.Warnings` on the `Harness` table `t`.
pub(super) fn register(lua: &Lua, harness: &Shared, t: &Table) -> LuaResult<()> {
    let h = Rc::clone(harness);
    t.set(
        "Warnings",
        lua.create_function(move |lua, after: Option<i64>| {
            let after = after.unwrap_or(0);
            let skip = usize::try_from(after).map_err(|_| {
                let msg = format!("Harness.Warnings: after must be a whole 0 or more, got {after}");
                mlua::Error::RuntimeError(msg)
            })?;
            let console = Rc::clone(&h.borrow().console);
            rusty_logging::forward(&console);
            let console = console.borrow();
            let lines = lua.create_table()?;
            for (text, level) in console.warnings().iter().skip(skip) {
                let line = lua.create_table()?;
                line.set("level", level_name(level))?;
                line.set("text", text.as_str())?;
                lines.push(line)?;
            }
            Ok(lines)
        })?,
    )
}

/// How `Harness.Warnings` names a level, as the demo index does.
fn level_name(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "error",
        LogLevel::Warning => "warning",
        LogLevel::Info => "info",
    }
}
