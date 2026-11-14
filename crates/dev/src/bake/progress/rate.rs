//! A step's rate over its recent past (#1890): the ETA divides the work left by the
//! work done over the last [`WINDOW`], not since the step began.
//!
//! A lightmap's texels are no uniform unit of work: a texel in a lit interior pays
//! a shadow ray per lamp in reach, one under the open sky few. Kolvik's first cheap
//! meshes ran at ~317k texels/min and the rest at ~75k, so a rate since the start
//! put the ETA at 8 min and raised it past 50. Over a window the rate follows the
//! work the step is doing now, and an ETA lags a change of pace by one window at
//! most. Two minutes: a mesh traces in seconds, so it averages dozens of them, and
//! it is 24 log lines ([`INTERVAL`](super::INTERVAL)), yet short beside a lightmap
//! step that takes long enough to be worth an estimate.

use std::collections::VecDeque;
use std::time::Duration;

/// How far back a step's rate is measured.
pub const WINDOW: Duration = Duration::from_secs(120);

/// Samples kept at most this often: a probe step observes on every probe.
const SPACING: Duration = Duration::from_secs(1);

/// A step's (time, work done) samples, the oldest at most one beyond [`WINDOW`].
#[derive(Default)]
pub(super) struct Window {
    samples: VecDeque<(Duration, u64)>,
}

impl Window {
    /// Note the work done at `now`; samples older than the window's start go.
    pub(super) fn note(&mut self, now: Duration, done: u64) {
        let spaced = self
            .samples
            .back()
            .is_none_or(|&(at, _)| now.saturating_sub(at) >= SPACING);
        if spaced {
            self.samples.push_back((now, done));
        }
        let start = now.saturating_sub(WINDOW);
        // Keep the newest sample at or before the start, so a full window spans it.
        while self.samples.get(1).is_some_and(|&(at, _)| at <= start) {
            self.samples.pop_front();
        }
    }

    /// Where to measure the rate from: a window back, or the step's first sample
    /// while the step is younger than that; `None` before any sample.
    pub(super) fn from(&self) -> Option<(Duration, u64)> {
        self.samples.front().copied()
    }
}
