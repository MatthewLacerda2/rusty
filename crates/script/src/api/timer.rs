//! crates/script/src/api/timer.rs — `Timer` namespace (#444).
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

use mlua::{Function, Lua, MultiValue, Table, Value};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::{require_sim_vm, wait, ConsoleLogs, Target, TimerScheduler, Wait, Work};
use rusty_scene::Scene;

/// Register the `Timer` namespace onto `lua`.
pub fn register(
    lua: &Lua,
    scene: ApiCell<Scene>,
    timers: ApiCell<TimerScheduler>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_invoke(lua, &table, scene, timers)?;
    register_handles(lua, &table, timers)?;
    register_coroutines(lua, &table, scene, timers, console)?;
    register_waits(lua, &table)?;
    lua.globals().set("Timer", table).map_err(|e| e.to_string())
}

/// `Invoke` / `InvokeRepeating`, then the queries.
fn register_invoke(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    timers: ApiCell<TimerScheduler>,
) -> Reg {
    let schedule = move |lua: &Lua, id: u32, target: Value, delay: f64, interval: Option<f64>| {
        let verb = ["Timer.Invoke", "Timer.InvokeRepeating"][usize::from(interval.is_some())];
        let (scene, timers) = (scene.get(lua)?, timers.get(lua)?);
        require_active(&scene.borrow(), id)?;
        let target = to_target(lua, target, verb)?;
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
        lua.create_function(move |lua, (id, target, delay): (EntityArg, Value, f64)| {
            let id = id.live("Timer", "Invoke")?;
            schedule(lua, id, target, delay, None)
        }),
    )?;
    put(
        table,
        "InvokeRepeating",
        lua.create_function(
            move |lua, (id, target, delay, every): (EntityArg, Value, f64, f64)| {
                let id = id.live("Timer", "InvokeRepeating")?;
                if every <= 0.0 {
                    return Err(runtime("InvokeRepeating interval must be > 0"));
                }
                schedule(lua, id, target, delay, Some(every))
            },
        ),
    )?;
    register_invoke_queries(lua, table, timers)
}

/// `CancelInvoke` / `IsInvoking`.
fn register_invoke_queries(lua: &mlua::Lua, table: &Table, timers: ApiCell<TimerScheduler>) -> Reg {
    put(
        table,
        "CancelInvoke",
        lua.create_function(move |lua, (id, name): (EntityArg, Option<String>)| {
            let id = id.live("Timer", "CancelInvoke")?;
            timers
                .get(lua)?
                .borrow_mut()
                .cancel_invokes(id, name.as_deref());
            Ok(())
        }),
    )?;
    put(
        table,
        "IsInvoking",
        lua.create_function(move |lua, (id, name): (EntityArg, Option<String>)| {
            Ok(timers
                .get(lua)?
                .borrow()
                .is_invoking(id.read(), name.as_deref()))
        }),
    )
}

/// `Cancel` / `IsPending`: the handle-level verbs, for invokes and coroutines alike.
fn register_handles(lua: &mlua::Lua, table: &Table, timers: ApiCell<TimerScheduler>) -> Reg {
    put(
        table,
        "Cancel",
        lua.create_function(move |lua, handle: u64| {
            Ok(timers.get(lua)?.borrow_mut().cancel(handle))
        }),
    )?;
    put(
        table,
        "IsPending",
        lua.create_function(move |lua, handle: u64| {
            Ok(timers.get(lua)?.borrow().get(handle).is_some())
        }),
    )
}

/// `StartCoroutine` / `StopAllCoroutines`. A started coroutine runs at once, up to
/// its first yield (Unity's contract), and the scheduler resumes it from there.
fn register_coroutines(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    timers: ApiCell<TimerScheduler>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "StartCoroutine",
        lua.create_function(
            move |lua, (id, f, args): (EntityArg, Function, MultiValue)| {
                timers.with(lua, |timers| {
                    let id = id.live("Timer", "StartCoroutine")?;
                    require_active(&scene.get(lua)?.borrow(), id)?;
                    require_sim_vm(lua, "Timer.StartCoroutine")?;
                    let thread = lua.create_thread(f)?;
                    let handle = timers.borrow_mut().alloc();
                    let mut call = args.into_vec();
                    call.insert(0, Value::Integer(i64::from(id)));
                    let first = match thread.resume::<MultiValue>(MultiValue::from_vec(call)) {
                        Err(e) => Err(e.to_string()),
                        Ok(_) if !thread.is_resumable() => return Ok(handle),
                        Ok(y) => wait::parse(lua, y.into_iter().next().unwrap_or(Value::Nil)),
                    };
                    match first {
                        Ok(next) => {
                            let key = lua.create_registry_value(thread)?;
                            let work = Work::Coroutine { thread: key };
                            timers.borrow_mut().insert(handle, id, work, next);
                        }
                        Err(e) => console
                            .get(lua)?
                            .borrow_mut()
                            .error(format!("[Lua Error] Coroutine on entity {id} failed: {e}")),
                    }
                    Ok(handle)
                })
            },
        ),
    )?;
    put(
        table,
        "StopAllCoroutines",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.live("Timer", "StopAllCoroutines")?;
            timers.get(lua)?.borrow_mut().cancel_coroutines(id);
            Ok(())
        }),
    )
}

/// The yield instructions. They read no engine state.
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
    if !scene.world.is_active_in_hierarchy(id) {
        return Err(runtime(&format!("entity {id} is inactive")));
    }
    Ok(())
}

/// A function, or the name of a function on the owner's scripts.
/// `verb` names the caller in the refusal of a function from another VM.
fn to_target(lua: &Lua, value: Value, verb: &str) -> mlua::Result<Target> {
    match value {
        Value::Function(f) => {
            require_sim_vm(lua, verb)?;
            Ok(Target::Func(lua.create_registry_value(f)?))
        }
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
