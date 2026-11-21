//! Stepped gain: a level that moves in flat steps at a regular period instead
//! of smoothly.
//!
//! **Folded, not followed.** A step of half a decibel is smaller than the
//! ripple a note's own waveform puts into any window short enough to resolve
//! a 60 Hz step, so steps are not looked for one at a time. Instead a stretch
//! is cut into segments at a candidate period and phase, and two things are
//! averaged over every segment: the change *across* each boundary (the second
//! half of one segment to the first half of the next) and the change *within*
//! each segment (its first half to its second). A staircase changes only
//! across; a smooth fade changes as much within as across; a note's pulse,
//! read at its own period, swings within as far as it swings back across; and
//! noise changes at random, which averages away. So a staircase is a stretch
//! where across is consistently one way, clear of the noise, and within is
//! near nothing.
//!
//! The candidates come from where the block-to-block steps repeat
//! ([`candidates`]), and each is refined to a twentieth of a block, because
//! a 60 Hz period is 33⅓ blocks and a whole-block error drifts a third of a
//! segment off in ten steps.

use super::Found;

/// The periods looked for, in blocks of half a millisecond: 5 ms (a gain set
/// every 256-sample buffer at 48 kHz) to 64 ms (a game at 16 frames a
/// second). Not shorter: a note's release read at one cycle of its own pitch
/// steps down once a cycle, and above 200 Hz a cycle is under 5 ms.
const SHORTEST: usize = 10;
const LONGEST: usize = 128;
/// A stretch, in blocks, and how far the next one starts after it.
const STRETCH: usize = 1024;
const HOP: usize = 512;
/// The fewest steps a window must hold to be judged.
const STEPS: usize = 6;
/// The fewest steps a staircase takes to be reported: a fade a game makes
/// lasts tenths of a second, a dozen steps and more at 60 Hz, where a sound's
/// own flutter can line up for a handful.
const REPORTED: usize = 10;
/// How many steps long a window is judged over.
const WINDOW_STEPS: usize = 12;
/// How strongly, as a share of the steps' own power, a lag must repeat to
/// be worth refining. Noise repeats at no lag this strongly.
const REPEATS: f64 = 0.08;
/// How many of the strongest candidate periods are refined.
const TRIED: usize = 3;
/// How finely a candidate is refined, in blocks, either side of it.
const FINE: f64 = 0.05;
/// The smallest step worth reporting, in decibels.
const STEP_DB: f64 = 0.25;
/// How large the change within a segment may be, as a share of the step.
const FLAT: f64 = 0.3;
/// The share of steps that must go the staircase's way.
const AGREE: f64 = 0.8;
/// How many standard errors clear of nothing the mean step must be.
const CLEAR: f64 = 5.0;
/// The step series' window either side, in blocks, used to find candidates.
const WINDOW: usize = 4;
/// How strongly the envelope's change must repeat to be the sound's pulse.
const PULSE: f64 = 0.4;
/// Under this mean square a stretch of blocks is silence and has no level.
const SILENT: f64 = 1e-7;

/// Every staircase in one channel's block energies, blocks of `block` frames.
pub(super) fn staircases(energy: &[f64], block: usize) -> Vec<Found> {
    let level = Levels::of(energy);
    let steps = steps(energy);
    let len = energy.len();
    // Stretches every HOP blocks, and one more ending exactly at the end of
    // the signal, so the last half-second is read like any other.
    let mut starts: Vec<usize> = (0..=len.saturating_sub(STRETCH)).step_by(HOP).collect();
    if starts.last() != Some(&len.saturating_sub(STRETCH)) {
        starts.push(len.saturating_sub(STRETCH));
    }
    let mut fits: Vec<Fit> = Vec::new();
    for (n, &start) in starts.iter().enumerate() {
        let end = (start + STRETCH).min(len);
        // Judged over windows a dozen steps long, so a staircase much shorter
        // than the stretch it was noticed in is still the whole of a window.
        // Each stretch judges the windows that start before the next stretch
        // does; the last judges the rest, and one window ending at the end.
        let own = starts.get(n + 1).copied().unwrap_or(len);
        for lag in candidates(&steps[start..end]) {
            let long = WINDOW_STEPS * lag;
            let mut froms: Vec<usize> = (start..own).step_by((long / 2).max(1)).collect();
            if own == len {
                froms.push(len.saturating_sub(long));
            }
            for from in froms.into_iter().filter(|from| from + STEPS * lag <= len) {
                fits.extend(refined(&level, from, (from + long).min(len), lag));
            }
        }
    }
    fits.sort_by_key(|fit| fit.first);
    let mut found: Vec<Fit> = Vec::new();
    for fit in fits {
        match found.last_mut() {
            Some(last) if last.continues(&fit) => last.join(fit),
            _ => found.push(fit),
        }
    }
    found
        .into_iter()
        .filter(|fit| fit.steps() >= REPORTED)
        .map(|fit| fit.found(block))
        .collect()
}

/// Each block's level, by prefix sums, so any stretch's mean is two lookups.
struct Levels(Vec<f64>);

impl Levels {
    fn of(energy: &[f64]) -> Self {
        let mut sums = Vec::with_capacity(energy.len() + 1);
        sums.push(0.0);
        for e in energy {
            sums.push(sums.last().copied().unwrap_or(0.0) + e);
        }
        Self(sums)
    }

    /// Whether the sound in `from..end` pulses on its own, faster than
    /// `period`: a voice, a note rich in harmonics, two notes beating.
    ///
    /// Such an envelope rises and falls every cycle, and when a cut lands a
    /// whole number of cycles apart, every segment cuts the pulses the same
    /// way and their count, against a fade, fakes a staircase exactly — and
    /// a real staircase under it is as large as the pulse's own noise. Its
    /// pulse and a staircase cannot be told apart at this resolution, so
    /// nothing is reported there. A pulse is a strong repeat in the
    /// envelope's block-to-block change at a lag under half the period,
    /// which a staircase on a steady sound, or on noise, does not have.
    fn pulsed(&self, from: usize, end: usize, period: f64) -> bool {
        let energy = |k: usize| self.0[k + 1] - self.0[k];
        let change: Vec<f64> = (from..end.saturating_sub(1))
            .map(|k| (energy(k + 1) - energy(k)) / (energy(k + 1) + energy(k) + SILENT))
            .collect();
        let power: f64 = change.iter().map(|d| d * d).sum();
        let repeat = |lag: usize| -> f64 {
            change
                .iter()
                .zip(&change[lag.min(change.len())..])
                .map(|(a, b)| a * b)
                .sum::<f64>()
                / power.max(f64::MIN_POSITIVE)
        };
        let longest = (period / 2.0) as usize;
        (2..=longest).any(|lag| repeat(lag) >= PULSE)
    }

    /// The level of blocks `from..to` in decibels, or `None` for silence.
    fn of_blocks(&self, from: usize, to: usize) -> Option<f64> {
        let mean = (self.0[to] - self.0[from]) / (to - from).max(1) as f64;
        (to > from && mean >= SILENT).then(|| 10.0 * mean.log10())
    }
}

/// One period and phase that fits a stretch as a staircase.
#[derive(Debug, Clone)]
struct Fit {
    period: f64,
    /// The mean change across a boundary, in decibels, signed.
    across: f64,
    within: f64,
    /// The first and last boundary that stepped the staircase's way.
    first: usize,
    last: usize,
    /// Every stretch joined into this one: its mean step, and its period.
    sizes: Vec<f64>,
    periods: Vec<f64>,
}

impl Fit {
    fn score(&self) -> f64 {
        self.across.abs() - self.within.abs()
    }

    /// How many steps the staircase takes, first to last.
    fn steps(&self) -> usize {
        ((self.last - self.first) as f64 / self.period).round() as usize + 1
    }

    fn continues(&self, next: &Fit) -> bool {
        next.first as f64 <= self.last as f64 + 2.0 * self.period
            && (next.period - self.period).abs() <= 0.03 * self.period.max(next.period)
            && next.across.signum() == self.across.signum()
    }

    fn join(&mut self, next: Fit) {
        self.first = self.first.min(next.first);
        self.last = self.last.max(next.last);
        self.sizes.extend(next.sizes);
        self.periods.extend(next.periods);
    }

    fn found(mut self, block: usize) -> Found {
        self.sizes.sort_by(f64::total_cmp);
        let period = self.periods.iter().sum::<f64>() / self.periods.len() as f64;
        Found {
            at: (self.first * block) as u64,
            from: (self.first * block) as u64,
            to: (self.last * block) as u64,
            size_db: self.sizes[self.sizes.len() / 2],
            period: Some(period * block as f64),
        }
    }
}

/// The best fit near a lag of `lag` blocks, if any fits at all: tried a
/// quarter of a block and two blocks of phase apart first, then refined
/// around the best of those.
fn refined(level: &Levels, start: usize, end: usize, lag: usize) -> Option<Fit> {
    let best = |tries: Vec<(f64, usize)>| {
        tries
            .into_iter()
            .filter_map(|(period, phase)| {
                Some((period, phase, fit(level, start + phase, end, period)?))
            })
            .max_by(|a, b| a.2.score().total_cmp(&b.2.score()))
    };
    let coarse: Vec<(f64, usize)> = (0..=8)
        .map(|n| lag as f64 - 1.0 + n as f64 * 0.25)
        .flat_map(|period| {
            (0..period.ceil() as usize)
                .step_by(2)
                .map(move |phase| (period, phase))
        })
        .collect();
    let (period, phase, _) = best(coarse)?;
    let fine: Vec<(f64, usize)> = (0..=10)
        .map(|n| period - 0.25 + n as f64 * FINE)
        .flat_map(|period| (phase.saturating_sub(1)..=phase + 1).map(move |phase| (period, phase)))
        .collect();
    best(fine).map(|(_, _, fit)| fit)
}

/// The stretch `from..end` cut into segments of `period` blocks, as a
/// staircase, if it is one.
///
/// Read twice: as cut, and cut half a step later. A staircase cut half a step
/// late has its steps in the middle of every segment, so what was across is
/// now within and nothing is across. A note whose pulse merely lines up with
/// the cut does not swap that way, and neither does noise.
fn fit(level: &Levels, from: usize, end: usize, period: f64) -> Option<Fit> {
    // Cheapest first: the halves alone turn almost every cut away.
    let cut = Cut::of(level, from, end, period)?;
    let step = cut.across.abs();
    let clear = step >= STEP_DB
        && cut.halves.abs() <= FLAT * step
        && cut.way.len() as f64 >= AGREE * cut.n
        && step >= CLEAR * cut.spread / cut.n.sqrt();
    if !clear {
        return None;
    }
    let late = Cut::of(level, from + (period / 2.0).round() as usize, end, period)?;
    let swapped =
        late.across.abs() <= FLAT * step && (late.halves - cut.across).abs() <= FLAT * step;
    if !swapped
        || quarters(level, from, cut.segments, period)?.abs() > FLAT * step
        || level.pulsed(from, end, period)
    {
        return None;
    }
    let at = |i: usize| from + ((i + 1) as f64 * period).round() as usize;
    Some(Fit {
        period,
        across: cut.across,
        within: cut.halves,
        first: at(cut.way[0]),
        last: at(cut.way[cut.way.len() - 1]),
        sizes: vec![step],
        periods: vec![period],
    })
}

/// The largest mean change between consecutive quarters of a segment: a
/// staircase is flat at any division of its step, where a note's pulse is
/// not.
fn quarters(level: &Levels, from: usize, segments: usize, period: f64) -> Option<f64> {
    let part = |i: usize, quarter: usize| {
        from + ((i as f64 + quarter as f64 / 4.0) * period).round() as usize
    };
    let mut changes = [0.0; 3];
    for i in 0..segments {
        let q = |n: usize| level.of_blocks(part(i, n), part(i, n + 1));
        let levels = [q(0)?, q(1)?, q(2)?, q(3)?];
        for (n, change) in changes.iter_mut().enumerate() {
            *change += levels[n + 1] - levels[n];
        }
    }
    changes
        .into_iter()
        .map(|change| change / segments as f64)
        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
}

/// One way of cutting a stretch into segments, measured by halves.
struct Cut {
    segments: usize,
    /// The mean change across a boundary, signed.
    across: f64,
    /// The mean change between a segment's halves.
    halves: f64,
    /// How far the changes across spread, and how many there are.
    spread: f64,
    n: f64,
    /// Which boundaries changed the way the mean does, by at least half of it.
    way: Vec<usize>,
}

impl Cut {
    fn of(level: &Levels, from: usize, end: usize, period: f64) -> Option<Cut> {
        let part = |i: usize, half: usize| {
            from + ((i as f64 + half as f64 / 2.0) * period).round() as usize
        };
        let segments = (end.checked_sub(from)? as f64 / period) as usize;
        if segments < STEPS {
            return None;
        }
        let levels: Vec<(f64, f64)> = (0..segments)
            .map(|i| {
                Some((
                    level.of_blocks(part(i, 0), part(i, 1))?,
                    level.of_blocks(part(i, 1), part(i, 2))?,
                ))
            })
            .collect::<Option<_>>()?;
        let halves = levels.iter().map(|(a, b)| b - a).sum::<f64>() / segments as f64;
        let steps: Vec<f64> = levels
            .windows(2)
            .map(|pair| pair[1].0 - pair[0].1)
            .collect();
        let n = steps.len() as f64;
        let across = steps.iter().sum::<f64>() / n;
        let spread = (steps.iter().map(|s| (s - across).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        let way = (0..steps.len())
            .filter(|&i| steps[i] * across > 0.0 && steps[i].abs() >= across.abs() / 2.0)
            .collect();
        Some(Cut {
            segments,
            across,
            halves,
            spread,
            n,
            way,
        })
    }
}

/// The lags, in blocks, at which the block-to-block steps repeat most
/// strongly: the strongest few local peaks of their autocorrelation.
fn candidates(steps: &[f64]) -> Vec<usize> {
    let longest = LONGEST.min(steps.len() / STEPS);
    if longest <= SHORTEST {
        return Vec::new();
    }
    let ac: Vec<f64> = (0..=longest + 1)
        .map(|lag| {
            steps
                .iter()
                .zip(&steps[lag.min(steps.len())..])
                .map(|(a, b)| a * b)
                .sum()
        })
        .collect();
    let mut peaks: Vec<usize> = (SHORTEST..=longest)
        .filter(|&lag| {
            ac[lag] >= REPEATS * ac[0] && ac[lag] >= ac[lag - 1] && ac[lag] >= ac[lag + 1]
        })
        .collect();
    peaks.sort_by(|&a, &b| ac[b].total_cmp(&ac[a]));
    peaks.truncate(TRIED);
    peaks
}

/// The step at each block boundary, in decibels: the level just after over
/// the level just before, each a Hann-weighted [`WINDOW`]. Zero where either
/// side is silent or too near an end.
fn steps(energy: &[f64]) -> Vec<f64> {
    let weights: Vec<f64> = (0..WINDOW)
        .map(|j| 0.5 - 0.5 * (std::f64::consts::TAU * (j as f64 + 0.5) / WINDOW as f64).cos())
        .collect();
    let side = |from: usize| -> f64 {
        weights
            .iter()
            .zip(&energy[from..from + WINDOW])
            .map(|(w, e)| w * e)
            .sum()
    };
    (0..energy.len())
        .map(|at| {
            if at < WINDOW || at + WINDOW > energy.len() {
                return 0.0;
            }
            let (before, after) = (side(at - WINDOW), side(at));
            if before < SILENT || after < SILENT {
                return 0.0;
            }
            10.0 * (after / before).log10()
        })
        .collect()
}

#[cfg(test)]
mod tests;
