//! A tween is one more kind of timer job, so it keeps every timer rule with no second copy.
//! Its `wait` is its delay; then it is due every tick, and `overshoot` is its clock.

pub(crate) mod ease;
mod field;
pub(crate) mod property;
mod shader_param;
mod step;

use glam::Vec4;
use mlua::RegistryKey;

pub(crate) use ease::Ease;
pub(crate) use property::Property;

use super::{TimerScheduler, Work, ELAPSED_EPSILON};

pub(crate) struct Tween {
    pub(crate) property: Property,
    pub(crate) from: Option<Vec4>,
    pub(crate) to: Vec4,
    pub(crate) duration: f64,
    pub(crate) ease: Ease,
    pub(crate) loops: Option<u32>,
    pub(crate) yoyo: bool,
    pub(crate) on_complete: Option<RegistryKey>,
    pub(crate) group: Option<u64>,
}

impl Tween {
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
    pub(crate) fn tween_mut(&mut self, handle: u64) -> Option<&mut Tween> {
        match &mut self.jobs.get_mut(&handle)?.work {
            Work::Tween(tween) => Some(tween),
            _ => None,
        }
    }

    pub(crate) fn take_tween(&mut self, handle: u64) -> Option<Box<Tween>> {
        match self.jobs.remove(&handle)?.work {
            Work::Tween(tween) => Some(tween),
            _ => None,
        }
    }

    pub(crate) fn kill_tweens(&mut self, handle: u64) -> bool {
        let before = self.jobs.len();
        self.jobs
            .retain(|&h, job| !tween_of(h, &job.work).is_some_and(|g| g.contains(&handle)));
        self.jobs.len() != before
    }

    pub(crate) fn kill_owner_tweens(&mut self, owner: u32) {
        self.jobs
            .retain(|_, job| !(job.owner == owner && matches!(job.work, Work::Tween(_))));
    }

    pub(crate) fn tween_playing(&self, handle: u64) -> bool {
        self.jobs
            .iter()
            .any(|(&h, job)| tween_of(h, &job.work).is_some_and(|g| g.contains(&handle)))
    }
}

fn tween_of(handle: u64, work: &Work) -> Option<[u64; 2]> {
    match work {
        Work::Tween(t) => Some([handle, t.group.unwrap_or(handle)]),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
