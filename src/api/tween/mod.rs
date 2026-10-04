//! src/api/tween/ — `Tween` namespace (#424).
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
use crate::scene::Scene;
use crate::scripting::{TimerScheduler, Wait, Work};
use spec::{Args, Spec};

/// Register the `Tween` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    timers: &'scope RefCell<TimerScheduler>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "To",
        scope.create_function(
            move |lua, (id, path, target, duration, opts): (u32, String, Value, f64, Option<Table>)| {
                let args = Args { id, path, target, duration };
                let spec = spec::parse(lua, &scene.borrow(), args, opts, false)?;
                Ok(schedule(scene, timers, spec, None))
            },
        ),
    )?;
    put(
        &table,
        "Sequence",
        scope.create_function(move |lua, steps: Table| {
            let specs = sequence(lua, &scene.borrow(), steps)?;
            let group = timers.borrow_mut().alloc();
            for spec in specs {
                schedule(scene, timers, spec, Some(group));
            }
            Ok(group)
        }),
    )?;
    put(
        &table,
        "Kill",
        scope.create_function(|_, handle: u64| Ok(timers.borrow_mut().kill_tweens(handle))),
    )?;
    put(
        &table,
        "KillAll",
        scope.create_function(|_, id: u32| {
            timers.borrow_mut().kill_owner_tweens(id);
            Ok(())
        }),
    )?;
    put(
        &table,
        "IsPlaying",
        scope.create_function(|_, handle: u64| Ok(timers.borrow().tween_playing(handle))),
    )?;
    lua.globals().set("Tween", table).map_err(|e| e.to_string())
}

/// Start a validated tween: write its `from` at once (so a delayed fade-in is
/// already hidden while it waits), then schedule it behind its delay.
fn schedule(
    scene: &RefCell<Scene>,
    timers: &RefCell<TimerScheduler>,
    mut spec: Spec,
    group: Option<u64>,
) -> u64 {
    if let Some(from) = spec.tween.from {
        spec.tween
            .property
            .set(&mut scene.borrow_mut(), spec.owner, from);
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
