//! A `Gpu` emitter's step history: the renderer replays the last `DT_HISTORY` `dt`s one by one
//! and ages a jump past them by the running times.

use std::collections::VecDeque;

use super::DT_HISTORY;

#[derive(Clone, Debug)]
pub(super) struct History {
    pub(super) steps: u64,
    dts: VecDeque<f32>,
    /// The running time after each kept step and the one before them: one more than `dts`.
    times: VecDeque<f64>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            steps: 0,
            dts: VecDeque::new(),
            times: VecDeque::from([0.0]),
        }
    }
}

impl History {
    pub(super) fn push(&mut self, dt: f32) {
        self.steps += 1;
        if self.dts.len() == DT_HISTORY {
            self.dts.pop_front();
        }
        self.dts.push_back(dt);
        let time = self.time() + f64::from(dt);
        if self.times.len() == DT_HISTORY + 1 {
            self.times.pop_front();
        }
        self.times.push_back(time);
    }

    pub(super) fn dt_of(&self, step: u64) -> Option<f32> {
        let first = self.steps + 1 - self.dts.len() as u64;
        let at = step.saturating_sub(first).min(self.dts.len() as u64);
        self.dts.get(at as usize).or(self.dts.back()).copied()
    }

    /// The `f64` sum of every `dt`, added in step order, so it is the same on every run.
    pub(super) fn time(&self) -> f64 {
        self.times.back().copied().unwrap_or(0.0)
    }

    pub(super) fn time_at(&self, step: u64) -> Option<f64> {
        let oldest = self.steps.checked_sub(self.times.len() as u64 - 1)?;
        let at = step.checked_sub(oldest)?;
        self.times.get(at as usize).copied()
    }
}
