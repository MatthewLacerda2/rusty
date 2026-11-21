//! A click: one sample-to-sample jump far larger than every other jump around
//! it.
//!
//! **Every other jump, not the typical one.** A sawtooth jumps by its whole
//! height once a period, a drum's attack is a run of large jumps, and noise is
//! nothing but jumps; none of them clicks. What clicks is a jump that stands
//! alone — a sample-accurate cut in a tone, a buffer spliced at the wrong
//! place — so a jump is measured against the *largest* jump in the
//! [`REACH`] either side of it, which a regular waveform always has a twin of
//! and a click never does. Two samples either side are left out, so an
//! impulse (up then straight back down) is not its own twin.
//!
//! **Another click is not a twin.** A jump that itself stands out from the
//! jumps beside it counts as a neighbour only when the waveform either side
//! of it matches this one's — a sawtooth's next reset does, sample for
//! sample — so two clicks 10 ms apart, or a splice at every audio buffer,
//! each landing at a different point of the waveform, are each a click.

use std::collections::VecDeque;

use super::Found;

/// How far either side a jump is compared, in seconds. Thirty-five
/// milliseconds holds a full period of anything down to 28.6 Hz, below
/// zimmer's lowest bass note (C1, 32.7 Hz), so a bass sawtooth always finds
/// its next reset inside it.
const REACH: f64 = 0.035;

/// How far apart, as a share of the jump, the waveform either side of two
/// jumps may be and still be the same jump of a repeating waveform.
const SAME: f32 = 0.05;

/// How many samples either side of a jump are not its neighbours.
const OWN: usize = 2;

/// How many times larger than every neighbour a jump must be. Twelve decibels.
const RATIO: f32 = 4.0;

/// The smallest jump worth calling a click: −40 dBFS. Under it a jump is too
/// small to hear over anything.
const FLOOR: f32 = 0.01;

/// The largest neighbour a jump is ever compared against, from below, so a
/// click out of digital silence reads as a large finite number.
const QUIET: f32 = 1e-4;

/// Clicks closer together than this, in seconds, are one click: an impulse
/// is a jump up and a jump straight back.
const ONE_CLICK: f64 = 0.005;

/// The click search for one channel.
#[derive(Debug, Clone)]
pub(super) struct Clicks {
    reach: usize,
    one_click: u64,
    previous: Option<f32>,
    /// The size of the most recent jumps, oldest first: never more than
    /// twice [`REACH`] and the jump being judged.
    recent: VecDeque<f32>,
    /// The samples those jumps land on, and the three before the first:
    /// `samples[k + 3]` is the sample `recent[k]` lands on. Before the
    /// signal, the first sample is held.
    samples: VecDeque<f32>,
    /// Which sample [`Clicks::recent`]'s first jump lands on.
    first: u64,
    /// How many samples were fed.
    fed: u64,
    /// The click being gathered, which may yet be joined by a larger one.
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
        // The jump `reach` back now has all of its right-hand neighbours.
        let newest = self.fed - 1;
        if newest > self.reach as u64 {
            self.judge(newest - self.reach as u64);
        }
    }

    /// Every click, the jumps still short of their right-hand neighbours
    /// judged as if the signal ended here.
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

    /// Whether the jump onto sample `at` is a click.
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

    /// Whether the jump at `pos` is itself a standout: four times the jumps
    /// three samples either side of it, which no stretch of noise or of a
    /// drum's attack keeps up.
    fn stands_out(&self, pos: usize) -> bool {
        let jump = self.recent[pos];
        let beside = [pos.checked_sub(3), Some(pos + 3)];
        beside
            .into_iter()
            .flatten()
            .filter_map(|at| self.recent.get(at))
            .all(|other| jump >= RATIO * other)
    }

    /// Whether the waveform around the jumps at `here` and `there` is the
    /// same, as it is at every reset of a sawtooth: the sample three before
    /// each and the sample two after.
    fn same_shape(&self, here: usize, there: usize) -> bool {
        let tolerance = SAME * self.recent[here];
        let around = |pos: usize| (self.samples.get(pos), self.samples.get(pos + 5));
        match (around(here), around(there)) {
            ((Some(a), Some(b)), (Some(c), Some(d))) => {
                (a - c).abs() <= tolerance && (b - d).abs() <= tolerance
            }
            // A jump without the waveform either side yet is nobody's twin:
            // a twin is a claim, and an unseen one must not cancel a click.
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
