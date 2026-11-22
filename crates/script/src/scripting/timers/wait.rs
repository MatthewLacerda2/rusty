use mlua::{Lua, RegistryKey, Table, Value};

use super::Clock;

const TAG: &str = "__wait";
const SECONDS: &str = "seconds";
const REALTIME: &str = "realtime";
const FIXED: &str = "fixed";
const UNTIL: &str = "until";

pub(crate) enum Wait {
    Elapsed { delay: f64, clock: Clock },
    Until(RegistryKey),
}

impl Wait {
    pub(crate) fn scaled(delay: f64) -> Self {
        Wait::Elapsed {
            delay: delay.max(0.0),
            clock: Clock::Scaled,
        }
    }
}

pub(crate) fn seconds(lua: &Lua, t: f64) -> mlua::Result<Table> {
    tagged(lua, SECONDS, Value::Number(t))
}

pub(crate) fn realtime(lua: &Lua, t: f64) -> mlua::Result<Table> {
    tagged(lua, REALTIME, Value::Number(t))
}

pub(crate) fn fixed(lua: &Lua) -> mlua::Result<Table> {
    tagged(lua, FIXED, Value::Nil)
}

pub(crate) fn until(lua: &Lua, f: mlua::Function) -> mlua::Result<Table> {
    tagged(lua, UNTIL, Value::Function(f))
}

fn tagged(lua: &Lua, kind: &str, arg: Value) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set(TAG, kind)?;
    t.set("arg", arg)?;
    Ok(t)
}

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
