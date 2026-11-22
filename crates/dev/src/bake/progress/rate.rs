use std::collections::VecDeque;
use std::time::Duration;

pub const WINDOW: Duration = Duration::from_secs(120);

const SPACING: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct Window {
    samples: VecDeque<(Duration, u64)>,
}

impl Window {
    pub(super) fn note(&mut self, now: Duration, done: u64) {
        let spaced = self
            .samples
            .back()
            .is_none_or(|&(at, _)| now.saturating_sub(at) >= SPACING);
        if spaced {
            self.samples.push_back((now, done));
        }
        let start = now.saturating_sub(WINDOW);
        while self.samples.get(1).is_some_and(|&(at, _)| at <= start) {
            self.samples.pop_front();
        }
    }

    pub(super) fn from(&self) -> Option<(Duration, u64)> {
        self.samples.front().copied()
    }
}
