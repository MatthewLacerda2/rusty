use mlua::debug::{Debug, DebugEvent, HookTriggers};
use mlua::{Function, Lua, MultiValue, Table, VmState};
use std::collections::BTreeSet;

use rusty_script::api::{ApiCtx, Reg, RUST_HALVES};

use super::{Event, Top, RECORDER};

pub(super) fn register(lua: &Lua, _ctx: &ApiCtx) -> Reg {
    if !super::armed() {
        return Ok(());
    }
    wrap_globals(lua).map_err(|e| e.to_string())
}

pub(super) fn wrap_globals(lua: &Lua) -> mlua::Result<()> {
    let mut seen = BTreeSet::new();
    let mut tables = Vec::new();
    for pair in lua.globals().pairs::<String, mlua::Value>() {
        let (name, value) = pair?;
        let mlua::Value::Table(table) = value else {
            continue;
        };
        if name.starts_with(|c: char| c.is_ascii_uppercase()) && seen.insert(table.to_pointer()) {
            tables.push((name, table));
        }
    }
    let halves: Option<Table> = lua.named_registry_value(RUST_HALVES)?;
    for pair in halves.iter().flat_map(|h| h.pairs::<String, Table>()) {
        tables.push(pair?);
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
        let verb = key.split('/').next().unwrap_or(&key);
        let name = format!("{namespace}.{verb}");
        let id = RECORDER.with_borrow_mut(|r| r.intern(name));
        table.set(key, timed(lua, id, f)?)?;
    }
    Ok(())
}

fn timed(lua: &Lua, id: usize, f: Function) -> mlua::Result<Function> {
    lua.create_function(move |lua, args: MultiValue| {
        let called = super::now();
        let outermost = RECORDER.with_borrow(|r| r.stack.is_empty());
        if outermost {
            lua.set_global_hook(HookTriggers::ON_CALLS | HookTriggers::ON_RETURNS, hook)?;
        }
        let (at, entered) = RECORDER.with_borrow_mut(|r| r.enter(id));
        let out = f.call::<MultiValue>(args);
        let left = RECORDER.with_borrow_mut(|r| r.leave(at));
        if outermost {
            lua.remove_hook();
            lua.remove_global_hook();
        }
        let span = left.duration_since(entered);
        let own = super::now()
            .duration_since(called)
            .saturating_sub(span)
            .as_secs_f64()
            * 1000.0;
        RECORDER.with_borrow_mut(|r| r.overhead(own));
        out.map_err(unwrapped)
    })
}

fn unwrapped(e: mlua::Error) -> mlua::Error {
    match e {
        mlua::Error::CallbackError { cause, .. } => (*cause).clone(),
        e => e,
    }
}

fn hook(lua: &Lua, debug: &Debug) -> mlua::Result<VmState> {
    let event = match debug.event() {
        DebugEvent::Call => Event::Call,
        DebugEvent::Ret => Event::Return,
        _ => return Ok(VmState::Continue),
    };
    let reenters = || debug.source().what != "C";
    match (RECORDER.with_borrow(super::Recorder::top), event) {
        (Top::Verb, Event::Call) if reenters() => {
            let depth = depth(lua);
            RECORDER.with_borrow_mut(|r| r.open_lua(depth));
        }
        (Top::Lua(_), _) => {
            let depth = depth(lua);
            let closed = RECORDER.with_borrow_mut(|r| r.lua_event(depth));
            if closed && event == Event::Call && reenters() {
                RECORDER.with_borrow_mut(|r| r.open_lua(depth));
            }
        }
        _ => {}
    }
    Ok(VmState::Continue)
}

fn depth(lua: &Lua) -> usize {
    let has = |level: usize| lua.inspect_stack(level, |_| ()).is_some();
    let (mut lo, mut hi) = (0, 1);
    while has(hi) {
        (lo, hi) = (hi, hi * 2);
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if has(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo + 1
}
