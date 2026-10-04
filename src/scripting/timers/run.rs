//! The per-tick timer phase (#444): step the scheduler and call into Lua.
//!
//! Runs inside the script system right after every `Update`, so it sees this
//! tick's gameplay and settles before `LateUpdate` (Unity resumes `yield null` /
//! `WaitForSeconds` coroutines and fires `Invoke`s in the same slot). Jobs whose
//! owner is inactive or gone are dropped first — Unity stops coroutines when their
//! GameObject deactivates, and rusty applies that one rule to invokes too.

use mlua::{Function, Lua, MultiValue, Table, Thread, Value};

use super::super::manager::ScriptManager;
use super::{wait, Target, Wait, Work};

/// What a due job will do, pulled out of the scheduler so no borrow of it is held
/// while Lua runs (scripts re-enter the `Timer` namespace).
enum Step {
    Call {
        func: Option<Function>,
        name: Option<String>,
        interval: Option<f64>,
    },
    Resume {
        thread: Thread,
        until: Option<Function>,
    },
    /// A tween: sample and write its property (`tween/step.rs`).
    Tween,
}

impl ScriptManager {
    /// The timer phase: advance every pending timer and coroutine by this tick's
    /// scaled `dt` (or the unscaled step, for the realtime waits) and run the ones
    /// that came due, in ascending `(owner entity, handle)` order.
    pub fn tick_timers(&mut self, delta_time: f32) {
        let unscaled = self.time.borrow().unscaled_delta_time;
        self.timers
            .borrow_mut()
            .retain_owners(|id| self.entity_active(id) == Some(true));
        let due = self
            .timers
            .borrow_mut()
            .advance(f64::from(delta_time), f64::from(unscaled));
        if !due.is_empty() {
            self.with_api_scope(|lua| {
                for handle in due {
                    self.run_job(lua, handle);
                }
            });
        }
        self.timers.borrow_mut().end_phase();
    }

    /// Run one due job, if it is still pending and its owner still active (an
    /// earlier job this phase may have cancelled it or disabled its entity).
    fn run_job(&self, lua: &Lua, handle: u64) {
        let Some((owner, step)) = self.take_step(lua, handle) else {
            return;
        };
        if self.entity_active(owner) != Some(true) {
            self.timers.borrow_mut().cancel(handle);
            return;
        }
        match step {
            Step::Call {
                func,
                name,
                interval,
            } => {
                let name = name.unwrap_or_default();
                let func = func.or_else(|| self.named_callback(lua, owner, &name));
                self.fire_invoke(handle, owner, func.ok_or(name), interval);
            }
            Step::Resume { thread, until } => self.resume(lua, handle, owner, thread, until),
            Step::Tween => self.step_tween(lua, handle, owner),
        }
    }

    fn take_step(&self, lua: &Lua, handle: u64) -> Option<(u32, Step)> {
        let timers = self.timers.borrow();
        let job = timers.get(handle)?;
        let step = match &job.work {
            Work::Invoke { target, interval } => {
                let (func, name) = match target {
                    Target::Func(key) => (lua.registry_value(key).ok(), None),
                    Target::Name(n) => (None, Some(n.clone())),
                };
                Step::Call {
                    func,
                    name,
                    interval: *interval,
                }
            }
            Work::Coroutine { thread } => Step::Resume {
                thread: lua.registry_value(thread).ok()?,
                until: match &job.wait {
                    Wait::Until(key) => lua.registry_value(key).ok(),
                    Wait::Elapsed { .. } => None,
                },
            },
            Work::Tween(_) => Step::Tween,
        };
        Some((job.owner, step))
    }

    /// Fire an invoke: call `target(owner)` (`Err` carries the name that did not
    /// resolve), then re-arm a repeating one (keeping its overshoot, so it never
    /// drifts) or retire a one-shot.
    fn fire_invoke(
        &self,
        handle: u64,
        owner: u32,
        func: Result<Function, String>,
        interval: Option<f64>,
    ) {
        let func = match func {
            Ok(func) => func,
            Err(name) => {
                let msg = format!("no function `{name}` on its scripts");
                self.log_error(owner, "Invoke", &msg);
                self.timers.borrow_mut().cancel(handle);
                return;
            }
        };
        if let Err(e) = func.call::<()>(owner) {
            self.log_error(owner, "Invoke", &e.to_string());
        }
        let mut timers = self.timers.borrow_mut();
        match interval {
            Some(every) => {
                let carry = timers.overshoot(handle);
                timers.rearm(handle, Wait::scaled(every), carry);
            }
            None => {
                timers.cancel(handle);
            }
        }
    }

    /// The first function named `name` on `owner`'s scripts, in script-index order.
    fn named_callback(&self, lua: &Lua, owner: u32, name: &str) -> Option<Function> {
        self.entity_scripts
            .range((owner, 0)..=(owner, usize::MAX))
            .filter_map(|(_, inst)| lua.registry_value::<Table>(&inst.table).ok())
            .find_map(|t| t.get::<Function>(name).ok())
    }

    /// Resume a coroutine whose wait is over (asking its `WaitUntil` predicate
    /// first), then re-arm it on whatever it yields next, or retire it.
    fn resume(&self, lua: &Lua, handle: u64, owner: u32, thread: Thread, until: Option<Function>) {
        if let Some(pred) = until {
            match pred.call::<bool>(()) {
                Ok(false) => return,
                Ok(true) => {}
                Err(e) => return self.retire(handle, owner, &e.to_string()),
            }
        }
        match thread.resume::<MultiValue>(()) {
            Err(e) => self.retire(handle, owner, &e.to_string()),
            Ok(_) if !thread.is_resumable() => {
                self.timers.borrow_mut().cancel(handle);
            }
            Ok(yielded) => {
                let first = yielded.into_iter().next().unwrap_or(Value::Nil);
                match wait::parse(lua, first) {
                    Ok(next) => self.timers.borrow_mut().rearm(handle, next, 0.0),
                    Err(e) => self.retire(handle, owner, &e),
                }
            }
        }
    }

    /// Drop a coroutine that failed, surfacing the error like an `Update` error.
    fn retire(&self, handle: u64, owner: u32, err: &str) {
        self.timers.borrow_mut().cancel(handle);
        self.log_error(owner, "Coroutine", err);
    }

    pub(super) fn log_error(&self, owner: u32, what: &str, err: &str) {
        self.console.borrow_mut().error(format!(
            "[Lua Error] {what} on entity {owner} failed: {err}"
        ));
    }
}
