//! the Chamberlin state-variable filter.

use std::f32::consts::PI;

use crate::patch::{Filter, FilterKind};

const MIN_CUTOFF: f32 = 20.0;
const MAX_CUTOFF_RATIO: f32 = 1.0 / 3.0;

const MAX_PAIRS: usize = 2;

#[derive(Clone, Copy, Default)]
struct Svf {
    low: f32,
    band: f32,
}

impl Svf {
    fn step(&mut self, input: f32, f: f32, q: f32, kind: FilterKind) -> f32 {
        let high = input - self.low - q * self.band;
        self.band += f * high;
        self.low += f * self.band;
        if !self.low.is_finite() || !self.band.is_finite() {
            self.low = 0.0;
            self.band = 0.0;
            return 0.0;
        }
        match kind {
            FilterKind::Lowpass => self.low,
            FilterKind::Highpass => high,
            // Already computed as state and, until now, thrown away every sample.
            FilterKind::Bandpass => self.band,
            FilterKind::Notch => high + self.low,
        }
    }
}

fn cascade(pairs: &mut [Svf], input: f32, f: f32, q: f32, kind: FilterKind) -> f32 {
    pairs
        .iter_mut()
        .fold(input, |signal, pair| pair.step(signal, f, q, kind))
}

pub(crate) fn apply(buf: &mut [f32], filter: &Filter, cutoffs: &[f32], sample_rate: f32) {
    let mut svf = [Svf::default(); MAX_PAIRS];
    let pairs = &mut svf[..filter.slope.pole_pairs()];
    let q = (1.0 - filter.resonance.clamp(0.0, 0.99)).clamp(0.05, 1.0);
    let ceiling = sample_rate * MAX_CUTOFF_RATIO;
    // The two input samples before this one, as they were before filtering.
    let (mut before, mut last) = (0.0, 0.0);
    for i in 0..buf.len() {
        let current = buf[i];
        // Past the end, the parabola through the last three samples carries on: a zero there would
        // be a step, and a step is broadband.
        let next = buf
            .get(i + 1)
            .copied()
            .unwrap_or(3.0 * (current - last) + before);
        let cutoff = cutoffs.get(i).copied().unwrap_or(filter.cutoff);
        let f = tuning(cutoff.clamp(MIN_CUTOFF, ceiling), sample_rate);
        cascade(
            pairs,
            halfway(before, last, current, next),
            f,
            q,
            filter.kind,
        );
        buf[i] = cascade(pairs, current, f, q, filter.kind);
        (before, last) = (last, current);
    }
}

#[inline]
fn halfway(a: f32, b: f32, c: f32, d: f32) -> f32 {
    (9.0 * (b + c) - (a + d)) / 16.0
}

#[inline]
fn tuning(cutoff: f32, sample_rate: f32) -> f32 {
    (2.0 * (PI * cutoff / (2.0 * sample_rate)).sin()).min(1.0)
}

#[cfg(test)]
mod tests;
