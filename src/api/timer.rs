//! src/api/timer.rs — `Timer` namespace (#444).
//!
//! Unity's MonoBehaviour timing conveniences — `Invoke` / `InvokeRepeating` /
//! `CancelInvoke` / `IsInvoking`, `StartCoroutine` / `StopAllCoroutines` and the
//! `WaitForSeconds` / `WaitForSecondsRealtime` / `WaitForFixedUpdate` /
//! `WaitUntil` yield instructions — plus `Cancel` / `IsPending` on the handle
//! every start returns. They are a namespace rather than bare globals so the one
//! API surface stays one set of namespaces: the REPL, bot-players and scripts all
//! reach them the same way, and the doc-drift gate (#280) can see them.
//!
//! Backed by the scripting layer's [`TimerScheduler`]; the scheduler is stepped
//! once per fixed tick right after `Update`, so everything here is deterministic.

use std::cell::RefCell;

use mlua::{Function, Lua, MultiValue, Table, ThreadStatus, Value};

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::{wait, ConsoleLogs, Target, TimerScheduler, Wait, Work};

/// Register the `Timer` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    timers: &'scope RefCell<TimerScheduler>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_invoke(scope, &table, scene, timers)?;
    register_handles(scope, &table, timers)?;
    register_coroutines(scope, &table, scene, timers, console)?;
    register_waits(lua, &table)?;
    lua.globals().set("Timer", table).map_err(|e| e.to_string())
}

/// `Invoke` / `InvokeRepeating` / `CancelInvoke` / `IsInvoking`.
fn register_invoke<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    timers: &'scope RefCell<TimerScheduler>,
) -> Reg {
    let schedule = move |lua: &Lua, id: u32, target: Value, delay: f64, interval: Option<f64>| {
        require_active(&scene.borrow(), id)?;
        let target = to_target(lua, target)?;
        let mut t = timers.borrow_mut();
        let handle = t.alloc();
        t.insert(
            handle,
            id,
            Work::Invoke { target, interval },
            Wait::scaled(delay),
        );
        Ok(handle)
    };
    put(
        table,
        "Invoke",
        scope.create_function(move |lua, (id, target, delay): (u32, Value, f64)| {
            schedule(lua, id, target, delay, None)
        }),
    )?;
    put(
        table,
        "InvokeRepeating",
        scope.create_function(
            move |lua, (id, target, delay, every): (u32, Value, f64, f64)| {
                if every <= 0.0 {
                    return Err(runtime("InvokeRepeating interval must be > 0"));
                }
                schedule(lua, id, target, delay, Some(every))
            },
        ),
    )?;
    put(
        table,
        "CancelInvoke",
        scope.create_function(|_, (id, name): (u32, Option<String>)| {
            timers.borrow_mut().cancel_invokes(id, name.as_deref());
            Ok(())
        }),
    )?;
    put(
        table,
        "IsInvoking",
        scope.create_function(|_, (id, name): (u32, Option<String>)| {
            Ok(timers.borrow().is_invoking(id, name.as_deref()))
        }),
    )
}

/// `Cancel` / `IsPending`: the handle-level verbs, for invokes and coroutines alike.
fn register_handles<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    timers: &'scope RefCell<TimerScheduler>,
) -> Reg {
    put(
        table,
        "Cancel",
        scope.create_function(|_, handle: u64| Ok(timers.borrow_mut().cancel(handle))),
    )?;
    put(
        table,
        "IsPending",
        scope.create_function(|_, handle: u64| Ok(timers.borrow().get(handle).is_some())),
    )
}

/// `StartCoroutine` / `StopAllCoroutines`. A started coroutine runs at once, up to
/// its first yield (Unity's contract), and the scheduler resumes it from there.
fn register_coroutines<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    timers: &'scope RefCell<TimerScheduler>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "StartCoroutine",
        scope.create_function(move |lua, (id, f, args): (u32, Function, MultiValue)| {
            require_active(&scene.borrow(), id)?;
            let thread = lua.create_thread(f)?;
            let handle = timers.borrow_mut().alloc();
            let mut call = args.into_vec();
            call.insert(0, Value::Integer(i64::from(id)));
            let first = match thread.resume::<_, MultiValue>(MultiValue::from_vec(call)) {
                Err(e) => Err(e.to_string()),
                Ok(_) if thread.status() != ThreadStatus::Resumable => return Ok(handle),
                Ok(y) => wait::parse(lua, y.into_iter().next().unwrap_or(Value::Nil)),
            };
            match first {
                Ok(next) => {
                    let key = lua.create_registry_value(thread)?;
                    let work = Work::Coroutine { thread: key };
                    timers.borrow_mut().insert(handle, id, work, next);
                }
                Err(e) => console
                    .borrow_mut()
                    .error(format!("[Lua Error] Coroutine on entity {id} failed: {e}")),
            }
            Ok(handle)
        }),
    )?;
    put(
        table,
        "StopAllCoroutines",
        scope.create_function(|_, id: u32| {
            timers.borrow_mut().cancel_coroutines(id);
            Ok(())
        }),
    )
}

/// The yield instructions. They borrow no engine state, so they are plain
/// functions rather than scope-tied ones.
fn register_waits(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "WaitForSeconds",
        lua.create_function(|lua, t: f64| wait::seconds(lua, t)),
    )?;
    put(
        table,
        "WaitForSecondsRealtime",
        lua.create_function(|lua, t: f64| wait::realtime(lua, t)),
    )?;
    put(
        table,
        "WaitForFixedUpdate",
        lua.create_function(|lua, ()| wait::fixed(lua)),
    )?;
    put(
        table,
        "WaitUntil",
        lua.create_function(|lua, pred: Function| wait::until(lua, pred)),
    )
}

/// Timers and coroutines only start on an existing, active entity (Unity refuses
/// to start a coroutine on an inactive GameObject; rusty applies it to invokes too).
pub(super) fn require_active(scene: &Scene, id: u32) -> mlua::Result<()> {
    if !scene.world.contains(id) {
        return Err(runtime(&format!("no entity {id}")));
    }
    if !scene.world.is_active(id) {
        return Err(runtime(&format!("entity {id} is inactive")));
    }
    Ok(())
}

/// A function, or the name of a function on the owner's scripts.
fn to_target(lua: &Lua, value: Value) -> mlua::Result<Target> {
    match value {
        Value::Function(f) => Ok(Target::Func(lua.create_registry_value(f)?)),
        Value::String(s) => Ok(Target::Name(s.to_str()?.to_string())),
        other => Err(runtime(&format!(
            "expected a function or a function name, got {}",
            other.type_name()
        ))),
    }
}

pub(super) fn runtime(msg: &str) -> mlua::Error {
    mlua::Error::RuntimeError(msg.to_string())
}
