//! crates/script/src/api/tween/ — `Tween` namespace (#424).
//!
//! Animate a component property to a target over time on an easing curve:
//! `Tween.To(id, "CanvasGroup.alpha", 1, 0.3, { ease = "quad_out" })`. Plus
//! `Kill` / `KillAll` / `IsPlaying` on the handle, and `Sequence` for staggered
//! entrances. Tweens are jobs in the scripting layer's [`TimerScheduler`] (see
//! `scripting::timers::tween` for why), so they tick in the timer phase on the
//! sim's own `dt` — deterministic, and paused by `Time.SetTimeScale(0)` unless
//! `unscaled`. `spec.rs` reads and validates a tween out of Lua.

mod spec;

use std::cell::RefCell;

use mlua::{Lua, Table, Value};

use super::timer::runtime;
use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::{TimerScheduler, Wait, Work};
use rusty_scene::Scene;
use spec::{Args, Spec};

/// `Tween.To`'s `(id, path, target, duration, opts?)`.
type ToArgs = (EntityArg, String, Value, f64, Option<Table>);

/// Register the `Tween` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>, timers: ApiCell<TimerScheduler>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "To",
        lua.create_function(move |lua, (id, path, target, duration, opts): ToArgs| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Tween", "To")?;
            let args = Args {
                id,
                path,
                target,
                duration,
            };
            let spec = spec::parse(lua, &scene.borrow(), args, opts, false)?;
            Ok(schedule(scene, &*timers.get(lua)?, spec, None))
        }),
    )?;
    put(
        &table,
        "Sequence",
        lua.create_function(move |lua, steps: Table| {
            let scene = &*scene.get(lua)?;
            let timers = &*timers.get(lua)?;
            let specs = sequence(lua, &scene.borrow(), steps)?;
            let group = timers.borrow_mut().alloc();
            for spec in specs {
                schedule(scene, timers, spec, Some(group));
            }
            Ok(group)
        }),
    )?;
    register_control(lua, &table, timers)?;
    lua.globals().set("Tween", table).map_err(|e| e.to_string())
}

/// `Kill`, `KillAll` and `IsPlaying`: the running tweens, by handle or owner.
fn register_control(lua: &Lua, table: &Table, timers: ApiCell<TimerScheduler>) -> Reg {
    put(
        table,
        "Kill",
        lua.create_function(move |lua, handle: u64| {
            Ok(timers.get(lua)?.borrow_mut().kill_tweens(handle))
        }),
    )?;
    put(
        table,
        "KillAll",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.live("Tween", "KillAll")?;
            timers.get(lua)?.borrow_mut().kill_owner_tweens(id);
            Ok(())
        }),
    )?;
    put(
        table,
        "IsPlaying",
        lua.create_function(move |lua, handle: u64| {
            Ok(timers.get(lua)?.borrow().tween_playing(handle))
        }),
    )
}

/// Start a validated tween: write its `from` at once (so a delayed fade-in is
/// already hidden while it waits) and, for a clamped field, start from the value
/// the field then holds; then schedule it behind its delay.
fn schedule(
    scene: &RefCell<Scene>,
    timers: &RefCell<TimerScheduler>,
    mut spec: Spec,
    group: Option<u64>,
) -> u64 {
    if let Some(from) = spec.tween.from {
        let mut scene = scene.borrow_mut();
        spec.tween.property.set(&mut scene, spec.owner, from);
        // The start is what the op held, not the number given (#1418): an alpha
        // tween `from = 500` starts at 1, as the field reads. Only a clamped
        // field: a rotation reads back through Euler angles, so `from = 360`
        // read back would be 0 and a full turn would not turn.
        if spec.tween.property.is_held() {
            spec.tween.from = spec.tween.property.get(&scene, spec.owner).or(Some(from));
        }
    }
    spec.tween.group = group;
    let mut t = timers.borrow_mut();
    let handle = t.alloc();
    let wait = Wait::Elapsed {
        delay: spec.delay,
        clock: spec.clock,
    };
    t.insert(handle, spec.owner, Work::Tween(Box::new(spec.tween)), wait);
    handle
}

/// Lay a `Tween.Sequence` out on one timeline: a number is a gap, a table is a
/// tween `{id, property, target, duration, opts...}` that starts when everything
/// before it has ended — or, with `join = true`, when the previous tween started.
/// Every item is validated before any is scheduled.
fn sequence(lua: &Lua, scene: &Scene, steps: Table) -> mlua::Result<Vec<Spec>> {
    let (mut cursor, mut last_start) = (0.0_f64, 0.0_f64);
    let mut specs = Vec::new();
    for (i, step) in steps.sequence_values::<Value>().enumerate() {
        let item = match step? {
            Value::Integer(gap) if gap >= 0 => {
                cursor += gap as f64;
                continue;
            }
            Value::Number(gap) if gap.is_finite() && gap >= 0.0 => {
                cursor += gap;
                continue;
            }
            Value::Table(item) => item,
            _ => {
                return Err(runtime(&format!(
                    "sequence step {}: a gap >= 0 or a tween table",
                    i + 1
                )))
            }
        };
        let args = Args {
            id: item.get(1)?,
            path: item.get(2)?,
            target: item.get(3)?,
            duration: item.get(4)?,
        };
        let mut spec = spec::parse(lua, scene, args, Some(item), true)?;
        let start = if spec.join { last_start } else { cursor };
        let span = spec.span().ok_or_else(|| {
            runtime(&format!(
                "sequence step {}: an endless tween never ends",
                i + 1
            ))
        })?;
        cursor = cursor.max(start + span);
        spec.delay += start;
        last_start = start;
        specs.push(spec);
    }
    Ok(specs)
}
