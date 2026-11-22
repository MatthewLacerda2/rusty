use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table};

use super::Shared;
use rusty_script::scripting::LogLevel;

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

/// `Harness.Warnings`' names, as `demo::index::level_name` writes them: change both together.
fn level_name(level: &LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "error",
        LogLevel::Warning => "warning",
        LogLevel::Info => "info",
    }
}
