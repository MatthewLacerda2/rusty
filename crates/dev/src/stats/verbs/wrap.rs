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
//! Only Rust functions are wrapped: a namespace function written in Lua runs
//! as the script's own time, and the verbs it calls are timed themselves. The
//! engine's Lua-written verbs keep their Rust halves in a table the wrapper
//! reaches ([`rusty_script::api::RUST_HALVES`]): `Particles.ForEach`'s read and
//! edit, `GetParticles`' read and `SetParticles`' write are each timed under
//! the verb they serve, and the Lua loop around them (`particle/live.lua`,
//! calling the script's `fn`) is `script.lua`'s. Methods on userdata (a
//! handle's) are not wrapped; their time is `script.lua`'s too.
//!
//! The hook is set VM-wide (`set_global_hook`) while the outermost verb is open,
//! so a coroutine a verb starts (`Timer.StartCoroutine` runs its body to the
//! first yield) is watched too. A yield is no return: Lua a verb's coroutine runs
//! until it yields closes when the verb does, so the verb's work after that
//! resume reads as Lua.

use mlua::debug::{Debug, DebugEvent, HookTriggers};
use mlua::{Function, Lua, MultiValue, Table, VmState};
use std::collections::BTreeSet;

use rusty_script::api::{ApiCtx, Reg, RUST_HALVES};

use super::{Event, Top, RECORDER};

/// Wrap every function of every engine namespace on `lua`: each global table
/// whose name starts with a capital (Lua's own tables are lowercase). Nothing
/// unless this thread armed ([`super::armed`]).
pub(super) fn register(lua: &Lua, _ctx: &ApiCtx) -> Reg {
    if !super::armed() {
        return Ok(());
    }
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
        // A Rust half (`ForEach/edit`) is timed as the verb it serves.
        let verb = key.split('/').next().unwrap_or(&key);
        let name = format!("{namespace}.{verb}");
        let id = RECORDER.with_borrow_mut(|r| r.intern(name));
        table.set(key, timed(lua, id, f)?)?;
    }
    Ok(())
}

/// `f` timed as verb `id`: the call hook is set while the outermost verb is open,
/// and what the wrapper spends outside the verb's own span is its overhead.
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

/// The error a verb raised, without the layer its call from here added.
fn unwrapped(e: mlua::Error) -> mlua::Error {
    match e {
        mlua::Error::CallbackError { cause, .. } => (*cause).clone(),
        e => e,
    }
}

/// The call hook while a verb is open: a Lua function a verb calls opens a
/// stretch of re-entered Lua (a main chunk too: anything but a Rust function);
/// with one open, each call and return is placed by its depth.
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

/// How many functions are on the running thread's Lua stack, the hooked one
/// included: found by doubling, then halving, the level asked for.
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
