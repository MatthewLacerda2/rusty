//! crates/dev/src/lua_surface/debug/frame_log.rs — `Debug.FrameLog`,
//! `Debug.SetLogChannels` and `Debug.GetLogChannels` (#2206): the rolling frame log
//! read back, and its channels switched.
//!
//! The read goes to the files (`frame_log::read`), so it answers during Play, after
//! Stop, and in a session that never wrote a frame (it reads what the editor
//! wrote). The channels are the project's (`Application::set_log_channels`), so a
//! switch is kept in `project.rusty` and the running log follows it next frame.

use std::path::Path;

use mlua::{Lua, Table, Value};

use crate::frame_log::{now_ms, read, LogQuery, LOGS_DIR};
use rusty_core::settings::logs::LogChannels;
use rusty_script::api::lua_json::to_lua;
use rusty_script::api::{put, ApiCtx, Reg};

/// The option keys `Debug.FrameLog` takes.
const OPTIONS: &[&str] = &["last_seconds", "over_ms", "channel", "limit"];

/// Register the three on the `Debug` table.
pub fn register(lua: &Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    put(
        table,
        "FrameLog",
        lua.create_function(|lua, opts: Option<Table>| {
            let query = query(opts)?;
            let found = read(Path::new(LOGS_DIR), &query, now_ms());
            let out = lua.create_table()?;
            out.set("path", LOGS_DIR)?;
            out.set("records", to_lua(lua, &found.records)?)?;
            out.set("skipped", found.skipped)?;
            Ok(out)
        }),
    )?;
    let app = ctx.application;
    put(
        table,
        "SetLogChannels",
        lua.create_function(move |lua, switches: Table| {
            let mut pairs = Vec::new();
            for pair in switches.pairs::<String, Value>() {
                let (name, on) = pair.map_err(|e| err(&format!("SetLogChannels: {e}")))?;
                let Value::Boolean(on) = on else {
                    let got = on.type_name();
                    return Err(err(&format!(
                        "SetLogChannels: {name} must be true or false, not a {got}"
                    )));
                };
                pairs.push((name, on));
            }
            let set = app.get(lua)?.borrow_mut().set_log_channels(&pairs);
            set.map_err(|e| err(&format!("SetLogChannels: {e}")))
        }),
    )?;
    put(
        table,
        "GetLogChannels",
        lua.create_function(move |lua, ()| {
            let channels = app.get(lua)?.borrow().log_settings().channels;
            let out = lua.create_table()?;
            for name in LogChannels::NAMES {
                out.set(name, channels.get(name).unwrap_or(false))?;
            }
            Ok(out)
        }),
    )
}

/// `Debug.FrameLog`'s options as a [`LogQuery`]; an unknown key or a bad value
/// raises.
fn query(opts: Option<Table>) -> mlua::Result<LogQuery> {
    let mut query = LogQuery::default();
    let Some(opts) = opts else {
        return Ok(query);
    };
    for pair in opts.pairs::<Value, Value>() {
        let name = pair?.0.to_string()?;
        if !OPTIONS.contains(&name.as_str()) {
            let known = OPTIONS.join(", ");
            return Err(err(&format!(
                "FrameLog: unknown option {name:?}; expected one of {known}"
            )));
        }
    }
    let positive = |key: &str| -> mlua::Result<Option<f64>> {
        match opts.get::<Option<f64>>(key)? {
            Some(v) if v.is_nan() || v < 0.0 => Err(err(&format!(
                "FrameLog: {key} must be a number of at least 0 (got {v})"
            ))),
            v => Ok(v),
        }
    };
    query.last_seconds = positive("last_seconds")?;
    query.over_ms = positive("over_ms")?;
    if let Some(limit) = positive("limit")? {
        if limit < 1.0 || (limit.fract() != 0.0 && limit.is_finite()) {
            return Err(err(&format!(
                "FrameLog: limit must be a whole number, at least 1 (got {limit})"
            )));
        }
        query.limit = if limit.is_finite() {
            limit as usize
        } else {
            usize::MAX
        };
    }
    query.channel = opts.get::<Option<String>>("channel")?;
    if let Some(ch) = &query.channel {
        if !LogChannels::NAMES.contains(&ch.as_str()) && ch != "dropped" {
            let known = LogChannels::NAMES.join(", ");
            return Err(err(&format!(
                "FrameLog: channel {ch:?} is none of {known}, dropped"
            )));
        }
    }
    Ok(query)
}

fn err(msg: &str) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Debug.{msg}"))
}

#[cfg(test)]
#[path = "frame_log_tests.rs"]
mod tests;
