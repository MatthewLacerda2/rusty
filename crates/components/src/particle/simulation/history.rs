use std::collections::VecDeque;

use super::DT_HISTORY;

#[derive(Clone, Debug)]
pub(super) struct History {
    pub(super) steps: u64,
    dts: VecDeque<f32>,
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

    pub(super) fn time(&self) -> f64 {
        self.times.back().copied().unwrap_or(0.0)
    }

    pub(super) fn time_at(&self, step: u64) -> Option<f64> {
        let oldest = self.steps.checked_sub(self.times.len() as u64 - 1)?;
        let at = step.checked_sub(oldest)?;
        self.times.get(at as usize).copied()
    }
}
