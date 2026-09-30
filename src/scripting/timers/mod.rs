//! src/scripting/timers/ — script timers and coroutines (#444).
//!
//! The deterministic scheduler behind the `Timer` namespace: Unity's `Invoke` /
//! `InvokeRepeating` and coroutines that yield `WaitForSeconds`-style instructions.
//! Every pending piece of work is a [`Job`] owned by one entity, and the scheduler
//! is stepped once per fixed tick, right after `Update` (`run.rs`), in ascending
//! `(owner entity, handle)` order — handles are allocated in start order, so the
//! order is a pure function of what the scripts did. Time is the tick's own clock
//! (the scaled `dt`, or the unscaled fixed step for the realtime variants), never
//! the wall clock.
//!
//! **The one-tick rule.** Work never runs in the tick it was scheduled in: a job
//! born in epoch `k` (this tick, before or during the timer phase) accumulates no
//! time until the next tick. So a delay `d` started on tick `N` fires on tick
//! `N + max(1, ceil(d / dt))`, and a bare `coroutine.yield()` resumes next tick.
//!
//! Split: this file holds the data and the bookkeeping; `wait.rs` the yield
//! instructions; `run.rs` the per-tick phase that calls into Lua; `tween/` the
//! property tweens (#424), which are one more kind of job.

mod run;
pub(crate) mod tween;
pub(crate) mod wait;

use std::collections::BTreeMap;

use mlua::RegistryKey;

pub(crate) use wait::Wait;

/// Float tolerance on "has the delay elapsed": a 0.05 s delay at a 1/60 step must
/// fire on the 3rd tick even though three f32 steps sum to a hair under 0.05.
const ELAPSED_EPSILON: f64 = 1e-6;

/// Which clock a wait counts against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Clock {
    /// The tick's scaled `dt` — `Time.SetTimeScale(0)` freezes it.
    Scaled,
    /// The tick's unscaled step — keeps running through a time-scale pause.
    Unscaled,
}

/// What an `Invoke` calls: a Lua function, or the name of a function on one of
/// the owner's scripts (resolved when it fires, Unity's `Invoke("Name", t)`).
pub(crate) enum Target {
    Func(RegistryKey),
    Name(String),
}

/// The work a job does when its wait is over.
pub(crate) enum Work {
    /// `Invoke` / `InvokeRepeating`: call `target(owner)`; `interval` re-arms it.
    Invoke {
        target: Target,
        interval: Option<f64>,
    },
    /// A started coroutine: resume the Lua thread.
    Coroutine { thread: RegistryKey },
    /// A property tween (#424): due every tick once its delay (the wait) is over.
    Tween(Box<tween::Tween>),
}

/// One pending timer or suspended coroutine.
pub(crate) struct Job {
    pub(crate) owner: u32,
    pub(crate) work: Work,
    pub(crate) wait: Wait,
    /// Seconds accumulated against `wait` since it was armed.
    elapsed: f64,
    /// The scheduler epoch the wait was armed in — see the one-tick rule.
    born: u64,
}

/// The per-World scheduler: every pending timer and coroutine, by handle.
#[derive(Default)]
pub struct TimerScheduler {
    jobs: BTreeMap<u64, Job>,
    next_handle: u64,
    epoch: u64,
}

impl TimerScheduler {
    /// A fresh handle. Handles start at 1 and never repeat within a runtime, so a
    /// stale handle can never cancel somebody else's job.
    pub(crate) fn alloc(&mut self) -> u64 {
        self.next_handle += 1;
        self.next_handle
    }

    /// Schedule `work` for `owner` under `handle`, waiting on `wait`.
    pub(crate) fn insert(&mut self, handle: u64, owner: u32, work: Work, wait: Wait) {
        let born = self.epoch;
        let job = Job {
            owner,
            work,
            wait,
            elapsed: 0.0,
            born,
        };
        self.jobs.insert(handle, job);
    }

    /// Re-arm a still-pending job on a new wait (a coroutine's next yield, a
    /// repeating invoke's next interval). `carry` is time already past the old
    /// deadline, kept so a repeating timer does not drift.
    pub(crate) fn rearm(&mut self, handle: u64, wait: Wait, carry: f64) {
        let epoch = self.epoch;
        if let Some(job) = self.jobs.get_mut(&handle) {
            job.wait = wait;
            job.elapsed = carry;
            job.born = epoch;
        }
    }

    /// The job behind `handle`, if it is still pending.
    pub(crate) fn get(&self, handle: u64) -> Option<&Job> {
        self.jobs.get(&handle)
    }

    /// Cancel one job. `true` when it was pending.
    pub(crate) fn cancel(&mut self, handle: u64) -> bool {
        self.jobs.remove(&handle).is_some()
    }

    /// Drop every job whose owner fails `keep` — the entity went inactive or away.
    pub(crate) fn retain_owners(&mut self, keep: impl Fn(u32) -> bool) {
        self.jobs.retain(|_, job| keep(job.owner));
    }

    /// Cancel `owner`'s invokes — all of them, or only those naming `name`.
    pub(crate) fn cancel_invokes(&mut self, owner: u32, name: Option<&str>) {
        self.jobs
            .retain(|_, job| !(job.owner == owner && invoke_matches(&job.work, name)));
    }

    /// Cancel `owner`'s coroutines.
    pub(crate) fn cancel_coroutines(&mut self, owner: u32) {
        self.jobs
            .retain(|_, job| !(job.owner == owner && matches!(job.work, Work::Coroutine { .. })));
    }

    /// Whether `owner` has a pending invoke (naming `name`, when given).
    pub(crate) fn is_invoking(&self, owner: u32, name: Option<&str>) -> bool {
        self.jobs
            .values()
            .any(|job| job.owner == owner && invoke_matches(&job.work, name))
    }

    /// Forget everything — the Lua VM the jobs point into is being replaced.
    pub(crate) fn clear(&mut self) {
        self.jobs.clear();
    }

    /// Advance every job armed before this epoch by the tick's time and return the
    /// handles whose wait may be over, in `(owner, handle)` order. `WaitUntil` jobs
    /// are always returned — their predicate is the run phase's to ask.
    pub(crate) fn advance(&mut self, scaled: f64, unscaled: f64) -> Vec<u64> {
        let mut due = Vec::new();
        for (&handle, job) in &mut self.jobs {
            if job.born == self.epoch {
                continue;
            }
            let ready = match &job.wait {
                Wait::Elapsed { delay, clock } => {
                    job.elapsed += match clock {
                        Clock::Scaled => scaled,
                        Clock::Unscaled => unscaled,
                    };
                    job.elapsed + ELAPSED_EPSILON >= *delay
                }
                Wait::Until(_) => true,
            };
            if ready {
                due.push((job.owner, handle));
            }
        }
        due.sort_unstable();
        due.into_iter().map(|(_, handle)| handle).collect()
    }

    /// Close the tick: work scheduled from now on belongs to the next epoch.
    pub(crate) fn end_phase(&mut self) {
        self.epoch += 1;
    }

    /// Seconds a job has run past its `Elapsed` deadline (0 for anything else) —
    /// the carry a repeating invoke keeps.
    pub(crate) fn overshoot(&self, handle: u64) -> f64 {
        match self.jobs.get(&handle) {
            Some(Job {
                wait: Wait::Elapsed { delay, .. },
                elapsed,
                ..
            }) => (elapsed - delay).max(0.0),
            _ => 0.0,
        }
    }

    /// Whether nothing is pending.
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

/// Whether `work` is an invoke matching `name` (any invoke when `name` is `None`;
/// only name-targeted invokes can match a name).
fn invoke_matches(work: &Work, name: Option<&str>) -> bool {
    match (work, name) {
        (Work::Invoke { .. }, None) => true,
        (
            Work::Invoke {
                target: Target::Name(n),
                ..
            },
            Some(want),
        ) => n == want,
        _ => false,
    }
}
