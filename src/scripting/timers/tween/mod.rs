//! Tweens (#424): a component property driven to a target over time on an easing
//! curve — UI fades, slides and pops, door swings, light flickers, camera kicks.
//!
//! **A tween is one more kind of timer job** (`Work::Tween`), not a scheduler of
//! its own. It is the same kind of work as an `Invoke` — owned by an entity,
//! advanced once per fixed tick on the scaled (or unscaled) `dt`, run in
//! `(owner, handle)` order in the timer phase — so sharing the scheduler gives it
//! every rule timers already keep, with no second copy to drift: the one-tick
//! rule, the scaled/unscaled clocks, one handle space, and the lifetime —
//! deactivate, deferred destroy, a `Scene.Load` unload and a fresh VM all drop an
//! entity's tweens exactly where they drop its coroutines.
//!
//! The job's `wait` is the tween's `delay`; once it is over the job is due every
//! tick, and the time past the delay (`TimerScheduler::overshoot`) is the tween's
//! own clock. The run phase (`run.rs`) samples [`Tween::sample`] and writes the
//! value through [`Property::set`]; a finished tween retires and calls its
//! `on_complete`.

pub(crate) mod ease;
pub(crate) mod property;
mod step;

use glam::Vec4;
use mlua::RegistryKey;

pub(crate) use ease::Ease;
pub(crate) use property::Property;

use super::{TimerScheduler, Work, ELAPSED_EPSILON};

/// One running property tween.
pub(crate) struct Tween {
    pub(crate) property: Property,
    /// The start value: given (`opts.from`), or read from the component the tick
    /// the tween starts (after its delay).
    pub(crate) from: Option<Vec4>,
    pub(crate) to: Vec4,
    /// Seconds per cycle.
    pub(crate) duration: f64,
    pub(crate) ease: Ease,
    /// Cycles to play; `None` loops forever.
    pub(crate) loops: Option<u32>,
    /// Every other cycle plays backwards.
    pub(crate) yoyo: bool,
    pub(crate) on_complete: Option<RegistryKey>,
    /// The `Tween.Sequence` handle this tween belongs to.
    pub(crate) group: Option<u64>,
}

impl Tween {
    /// The value `t` seconds into the tween (after its delay), and whether it has
    /// finished. The last cycle lands exactly on its end value.
    pub(crate) fn sample(&self, from: Vec4, t: f64) -> (Vec4, bool) {
        let cycle_len = self.duration.max(0.0);
        let total = self.loops.map(|n| cycle_len * f64::from(n));
        if let Some(total) = total.filter(|total| t + ELAPSED_EPSILON >= *total) {
            let cycles = self.loops.unwrap_or(1);
            let back_home = self.yoyo && cycles.is_multiple_of(2) && total > 0.0;
            return (if back_home { from } else { self.to }, true);
        }
        if cycle_len <= 0.0 {
            return (self.to, false);
        }
        let cycle = (t / cycle_len).floor();
        let mut p = (t - cycle * cycle_len) / cycle_len;
        if self.yoyo && cycle % 2.0 == 1.0 {
            p = 1.0 - p;
        }
        let k = self.ease.apply(p) as f32;
        (from + (self.to - from) * k, false)
    }
}

impl TimerScheduler {
    /// The tween behind `handle`, if it is a pending tween.
    pub(crate) fn tween_mut(&mut self, handle: u64) -> Option<&mut Tween> {
        match &mut self.jobs.get_mut(&handle)?.work {
            Work::Tween(tween) => Some(tween),
            _ => None,
        }
    }

    /// Retire a finished tween, handing it back (for its `on_complete`).
    pub(crate) fn take_tween(&mut self, handle: u64) -> Option<Box<Tween>> {
        match self.jobs.remove(&handle)?.work {
            Work::Tween(tween) => Some(tween),
            _ => None,
        }
    }

    /// Kill the tween `handle`, or every tween of the sequence `handle`. `true`
    /// when anything was pending. Other kinds of job under `handle` are untouched.
    pub(crate) fn kill_tweens(&mut self, handle: u64) -> bool {
        let before = self.jobs.len();
        self.jobs
            .retain(|&h, job| !tween_of(h, &job.work).is_some_and(|g| g.contains(&handle)));
        self.jobs.len() != before
    }

    /// Kill every tween `owner` drives.
    pub(crate) fn kill_owner_tweens(&mut self, owner: u32) {
        self.jobs
            .retain(|_, job| !(job.owner == owner && matches!(job.work, Work::Tween(_))));
    }

    /// Whether the tween `handle` (or any tween of the sequence `handle`) is pending.
    pub(crate) fn tween_playing(&self, handle: u64) -> bool {
        self.jobs
            .iter()
            .any(|(&h, job)| tween_of(h, &job.work).is_some_and(|g| g.contains(&handle)))
    }
}

/// The handles a tween job answers to — its own and its sequence's — or `None`
/// for any other job.
fn tween_of(handle: u64, work: &Work) -> Option<[u64; 2]> {
    match work {
        Work::Tween(t) => Some([handle, t.group.unwrap_or(handle)]),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
