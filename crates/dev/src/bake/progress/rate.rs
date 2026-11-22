use std::collections::VecDeque;
use std::time::Duration;

/// Two minutes average dozens of meshes, yet stay short beside a lightmap step worth an ETA.
pub const WINDOW: Duration = Duration::from_secs(120);

/// Samples are kept at most this often: a probe step observes on every probe.
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
        // The newest sample at or before the start stays, so a full window spans it.
        while self.samples.get(1).is_some_and(|&(at, _)| at <= start) {
            self.samples.pop_front();
        }
    }

    pub(super) fn from(&self) -> Option<(Duration, u64)> {
        self.samples.front().copied()
    }
}
