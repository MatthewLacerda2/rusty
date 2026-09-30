//! The yield instructions a coroutine hands the scheduler (#444).
//!
//! `Timer.WaitForSeconds(t)` & co. return a small tagged table; the coroutine
//! yields it (`coroutine.yield(Timer.WaitForSeconds(0.5))`, Unity's
//! `yield return new WaitForSeconds(0.5)`), and [`parse`] turns the yielded value
//! back into a [`Wait`]. A bare `coroutine.yield()` means "next tick".

use mlua::{Lua, RegistryKey, Table, Value};

use super::Clock;

/// The field that tags a yield-instruction table, and its kinds.
const TAG: &str = "__wait";
const SECONDS: &str = "seconds";
const REALTIME: &str = "realtime";
const FIXED: &str = "fixed";
const UNTIL: &str = "until";

/// What a job is waiting on.
pub(crate) enum Wait {
    /// `delay` seconds on `clock` (0 = the next tick).
    Elapsed { delay: f64, clock: Clock },
    /// A Lua predicate, asked once per tick until it returns a truthy value.
    Until(RegistryKey),
}

impl Wait {
    /// `delay` seconds of scaled time (negative clamps to 0).
    pub(crate) fn scaled(delay: f64) -> Self {
        Wait::Elapsed {
            delay: delay.max(0.0),
            clock: Clock::Scaled,
        }
    }
}

/// `Timer.WaitForSeconds(t)`: `t` seconds of scaled time.
pub(crate) fn seconds<'lua>(lua: &'lua Lua, t: f64) -> mlua::Result<Table<'lua>> {
    tagged(lua, SECONDS, Value::Number(t))
}

/// `Timer.WaitForSecondsRealtime(t)`: `t` seconds of unscaled time.
pub(crate) fn realtime<'lua>(lua: &'lua Lua, t: f64) -> mlua::Result<Table<'lua>> {
    tagged(lua, REALTIME, Value::Number(t))
}

/// `Timer.WaitForFixedUpdate()`: the next fixed tick.
pub(crate) fn fixed(lua: &Lua) -> mlua::Result<Table<'_>> {
    tagged(lua, FIXED, Value::Nil)
}

/// `Timer.WaitUntil(fn)`: until `fn()` is truthy.
pub(crate) fn until<'lua>(lua: &'lua Lua, f: mlua::Function<'lua>) -> mlua::Result<Table<'lua>> {
    tagged(lua, UNTIL, Value::Function(f))
}

fn tagged<'lua>(lua: &'lua Lua, kind: &str, arg: Value<'lua>) -> mlua::Result<Table<'lua>> {
    let t = lua.create_table()?;
    t.set(TAG, kind)?;
    t.set("arg", arg)?;
    Ok(t)
}

/// Turn a coroutine's yielded value into a [`Wait`]. `nil` is "next tick"; a
/// `Timer.Wait*` table is its instruction; anything else is a script error.
pub(crate) fn parse(lua: &Lua, value: Value) -> Result<Wait, String> {
    let table = match value {
        Value::Nil => return Ok(Wait::scaled(0.0)),
        Value::Table(t) => t,
        other => return Err(unsupported(other.type_name())),
    };
    let kind: Option<String> = table.get(TAG).map_err(|e| e.to_string())?;
    let arg: Value = table.get("arg").map_err(|e| e.to_string())?;
    let secs = |arg: &Value| match arg {
        Value::Number(n) => Ok(n.max(0.0)),
        Value::Integer(i) => Ok((*i as f64).max(0.0)),
        _ => Err("wait duration must be a number".to_string()),
    };
    match (kind.as_deref(), arg) {
        (Some(SECONDS), a) => Ok(Wait::scaled(secs(&a)?)),
        (Some(REALTIME), a) => Ok(Wait::Elapsed {
            delay: secs(&a)?,
            clock: Clock::Unscaled,
        }),
        (Some(FIXED), _) => Ok(Wait::scaled(0.0)),
        (Some(UNTIL), Value::Function(f)) => lua
            .create_registry_value(f)
            .map(Wait::Until)
            .map_err(|e| e.to_string()),
        _ => Err(unsupported("table")),
    }
}

fn unsupported(what: &str) -> String {
    format!("coroutine yielded an unsupported {what}; yield nil or a Timer.Wait* instruction")
}
