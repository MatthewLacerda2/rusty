//! Wrapping the surface's verbs in their timers: the [`Extension`] [`super::arm`]
//! installs, run on every VM after every namespace (the dev ones included).
//!
//! [`Extension`]: rusty_script::api::Extension
//!
//! A wrapper is transparent: it takes and returns what the verb does, keeps its
//! name in its table (so `api_surface` and `tests/api_doc_drift.rs` see the same
//! surface), and hands a verb's error on as the verb raised it: mlua wraps a
//! Rust callback's error once, where it reaches Lua, so the wrapper unwraps the
//! one its inner call added and the script sees the same message and traceback.
//! One line differs: an error raised in Lua a verb called back (a handler
//! `Move` ran) is traced through the bare verb as well as its wrapper, so its
//! traceback has one more `[C]: in ?` frame, right above the verb's.
//! Only Rust functions are wrapped: a namespace function written in Lua is the
//! script's own time, and the verbs it calls are timed themselves. Methods on
//! userdata (a handle's) are not wrapped; their time is `script.lua`'s.

use std::collections::BTreeSet;

use mlua::debug::{Debug, DebugEvent, HookTriggers};
use mlua::{Function, Lua, MultiValue, Table, VmState};

use rusty_script::api::{ApiCtx, Reg};

use super::{Event, RECORDER};

/// Wrap every Rust function of every engine namespace on `lua`: each global
/// table whose name starts with a capital (Lua's own tables are lowercase).
pub(super) fn register(lua: &Lua, _ctx: &ApiCtx) -> Reg {
    wrap_globals(lua).map_err(|e| e.to_string())
}

/// [`register`] without the context: what the tests call on a bare VM.
pub(super) fn wrap_globals(lua: &Lua) -> mlua::Result<()> {
    let mut seen = BTreeSet::new();
    let mut tables = Vec::new();
    for pair in lua.globals().pairs::<String, mlua::Value>() {
        let (name, value) = pair?;
        let mlua::Value::Table(table) = value else {
            continue;
        };
        // A table two globals name (an alias) is wrapped once, under the first.
        if name.starts_with(|c: char| c.is_ascii_uppercase()) && seen.insert(table.to_pointer()) {
            tables.push((name, table));
        }
    }
    tables.sort_by(|a, b| a.0.cmp(&b.0));
    for (namespace, table) in tables {
        wrap_table(lua, &namespace, &table)?;
    }
    Ok(())
}

fn wrap_table(lua: &Lua, namespace: &str, table: &Table) -> mlua::Result<()> {
    let mut verbs = Vec::new();
    for pair in table.pairs::<String, mlua::Value>() {
        if let (key, mlua::Value::Function(f)) = pair? {
            if f.info().what == "C" {
                verbs.push((key, f));
            }
        }
    }
    verbs.sort_by(|a, b| a.0.cmp(&b.0));
    for (key, f) in verbs {
        let name = format!("{namespace}.{key}");
        let id = RECORDER.with_borrow_mut(|r| r.intern(name));
        table.set(key, timed(lua, id, f)?)?;
    }
    Ok(())
}

/// `f` timed as verb `id`: the call hook is set while the outermost verb is open.
fn timed(lua: &Lua, id: usize, f: Function) -> mlua::Result<Function> {
    lua.create_function(move |lua, args: MultiValue| {
        let outermost = RECORDER.with_borrow(|r| r.stack.is_empty());
        if outermost {
            lua.set_hook(HookTriggers::ON_CALLS | HookTriggers::ON_RETURNS, hook)?;
        }
        let at = RECORDER.with_borrow_mut(|r| r.enter(id));
        let out = f.call::<MultiValue>(args);
        RECORDER.with_borrow_mut(|r| r.leave(at));
        if outermost {
            lua.remove_hook();
        }
        out.map_err(unwrapped)
    })
}

/// The error a verb raised, without the layer its call from here added.
fn unwrapped(e: mlua::Error) -> mlua::Error {
    match e {
        mlua::Error::CallbackError { cause, .. } => (*cause).clone(),
        e => e,
    }
}

fn hook(_lua: &Lua, debug: &Debug) -> mlua::Result<VmState> {
    let event = match debug.event() {
        DebugEvent::Call => Event::Call {
            lua: debug.source().what == "Lua",
        },
        DebugEvent::Ret => Event::Return,
        _ => return Ok(VmState::Continue),
    };
    RECORDER.with_borrow_mut(|r| r.event(event));
    Ok(VmState::Continue)
}
