//! A click: one sample-to-sample jump far larger than every other jump around it.

use std::collections::VecDeque;

use super::Found;

/// Seconds either side: a full period down to 28.6 Hz, under C1 (32.7 Hz), so a bass saw finds
/// its next reset.
const REACH: f64 = 0.035;

const SAME: f32 = 0.05;

const OWN: usize = 2;

const RATIO: f32 = 4.0; // 12 dB

const FLOOR: f32 = 0.01; // -40 dBFS

const QUIET: f32 = 1e-4;

const ONE_CLICK: f64 = 0.005;

#[derive(Debug, Clone)]
pub(super) struct Clicks {
    reach: usize,
    one_click: u64,
    previous: Option<f32>,
    recent: VecDeque<f32>,
    samples: VecDeque<f32>,
    first: u64,
    fed: u64,
    pending: Option<Found>,
    found: Vec<Found>,
}

impl Clicks {
    pub(super) fn new(rate: u32) -> Self {
        Self {
            reach: ((REACH * f64::from(rate)) as usize).max(OWN + 1),
            one_click: (ONE_CLICK * f64::from(rate)) as u64,
            previous: None,
            recent: VecDeque::new(),
            samples: VecDeque::new(),
            first: 1,
            fed: 0,
            pending: None,
            found: Vec::new(),
        }
    }

    pub(super) fn feed(&mut self, sample: f32) {
        self.fed += 1;
        self.samples.push_back(sample);
        let Some(previous) = self.previous.replace(sample) else {
            // The first sample, held for the three before the first jump.
            self.samples.extend([sample; 2]);
            return;
        };
        self.recent.push_back((sample - previous).abs());
        if self.recent.len() > 2 * self.reach + 1 {
            self.recent.pop_front();
            self.first += 1;
        }
        // Three samples before the first jump's, as the window before it.
        while self.samples.len() > self.recent.len() + 3 {
            self.samples.pop_front();
        }
        let newest = self.fed - 1;
        if newest > self.reach as u64 {
            self.judge(newest - self.reach as u64);
        }
    }

    pub(super) fn finish(&self) -> Vec<Found> {
        let mut rest = self.clone();
        let newest = self.fed.saturating_sub(1);
        let from = (newest + 1).saturating_sub(self.reach as u64).max(1);
        for at in from..=newest {
            if self.fed >= 2 {
                rest.judge(at);
            }
        }
        rest.found.extend(rest.pending.take());
        rest.found
    }

    fn judge(&mut self, at: u64) {
        let Some(here) = at.checked_sub(self.first).map(|pos| pos as usize) else {
            return;
        };
        let jump = self.recent[here];
        if jump < FLOOR {
            return;
        }
        let neighbours = (here.saturating_sub(self.reach)..here.saturating_sub(OWN))
            .chain(here + OWN + 1..(here + self.reach + 1).min(self.recent.len()));
        let mut loudest = QUIET;
        for pos in neighbours {
            let other = self.recent[pos];
            if jump < RATIO * other && (!self.stands_out(pos) || self.same_shape(here, pos)) {
                return;
            }
            if jump >= RATIO * other {
                loudest = loudest.max(other);
            }
        }
        let size_db = 20.0 * f64::from(jump / loudest).log10();
        self.found_at(at, size_db);
    }

    fn stands_out(&self, pos: usize) -> bool {
        let jump = self.recent[pos];
        let beside = [pos.checked_sub(3), Some(pos + 3)];
        beside
            .into_iter()
            .flatten()
            .filter_map(|at| self.recent.get(at))
            .all(|other| jump >= RATIO * other)
    }

    fn same_shape(&self, here: usize, there: usize) -> bool {
        let tolerance = SAME * self.recent[here];
        let around = |pos: usize| (self.samples.get(pos), self.samples.get(pos + 5));
        match (around(here), around(there)) {
            ((Some(a), Some(b)), (Some(c), Some(d))) => {
                (a - c).abs() <= tolerance && (b - d).abs() <= tolerance
            }
            // A jump without the waveform either side yet is nobody's twin: a twin is a claim, and
            // an unseen one must not cancel a click.
            _ => false,
        }
    }

    fn found_at(&mut self, at: u64, size_db: f64) {
        let one = Found {
            at,
            from: at.saturating_sub(self.one_click),
            to: at + self.one_click,
            size_db,
            period: None,
        };
        match &mut self.pending {
            Some(pending) if at - pending.at <= self.one_click => {
                if size_db > pending.size_db {
                    *pending = one;
                }
            }
            _ => self.found.extend(self.pending.replace(one)),
        }
    }
}
