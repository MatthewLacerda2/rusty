//! the Chamberlin state-variable filter.
//!
//! The filter is what makes a subtractive patch expressive: a raw saw stack is a
//! buzz until an envelope sweeps a resonant cutoff across it. We use the
//! **Chamberlin SVF** — two integrators in a loop yielding lowpass, highpass and
//! bandpass from one topology, with an honest resonance knob — because it is stable
//! at audio rates, costs a handful of adds per sample, and takes a *new cutoff every
//! sample* for free (which is exactly what an envelope sweep and an LFO wobble need).
//! All four outputs the topology gives are exposed: the two it computes as state,
//! the high it computes on the way, and the notch, which is a single add of two of
//! them.
//!
//! A **slope** is that whole loop run twice in series. Both pairs take the same
//! tuning and the same damping, so 24 dB per octave is 12 applied twice — which is
//! also why resonance is a different control at the steeper setting: the emphasis
//! lands once per pair, so a peak is roughly squared and a filter that rang starts
//! to self-oscillate. That is the sound; it is also what a patch moved between the
//! two slopes has to be re-tuned for.
//!
//! The classic caveat is that the plain form goes unstable as the cutoff approaches
//! a quarter of the sample rate. We run it **2× oversampled** (two half-steps per
//! output sample), which pushes the stable ceiling up to ~`sr/3`, and clamp the
//! cutoff there — so a "fully open" filter really is open across the audible band.
//! The half-step between two samples is interpolated, never held: [`halfway`] has
//! why a highpass needs that.
//!
//! **That ceiling still holds for two pairs in series, and it was checked rather
//! than assumed.** Stability is a property of one integrator loop — the pole
//! positions of a pair depend on its own `f` and `q` and on nothing upstream of it
//! — so a second pair at the same coefficients is stable exactly where the first
//! is, and a cascade of stable stages is stable. What a cascade *does* change is
//! amplitude: a resonant peak passed through a second resonant peak comes out
//! roughly squared, which is loud rather than divergent, and the bake's limiter is
//! where loud is handled. `cascading_at_the_ceiling_stays_finite` pins both halves
//! of that at the top of the band with the resonance knob at its maximum.

use std::f32::consts::PI;

use crate::patch::{Filter, FilterKind};

/// Lowest cutoff a track is clamped to, in Hz — below hearing, and far from the
/// zero that would stall the integrators.
const MIN_CUTOFF: f32 = 20.0;
/// Highest cutoff as a fraction of the sample rate — the 2×-oversampled SVF's
/// stability ceiling.
const MAX_CUTOFF_RATIO: f32 = 1.0 / 3.0;

/// The most pole pairs any slope runs in series — [`Slope::Db24`]'s two.
///
/// [`Slope::Db24`]: crate::patch::Slope::Db24
const MAX_PAIRS: usize = 2;

/// Two integrator states — one pole pair's whole memory.
#[derive(Clone, Copy, Default)]
struct Svf {
    low: f32,
    band: f32,
}

impl Svf {
    /// One half-step of the SVF at the given tuning `f` and damping `q`, returning
    /// the requested output. Non-finite state (a numerical blow-up from an extreme
    /// setting) resets to silence rather than poisoning the rest of the buffer.
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
            // And the notch is one add of the two ends the pair already has.
            FilterKind::Notch => high + self.low,
        }
    }
}

/// One half-step through every pole pair in series, each taking the one before it.
///
/// Each pair reads the same `kind`, so two lowpasses make a steeper lowpass and two
/// notches a deeper one — the slope is the shape applied twice, never two different
/// shapes stacked.
fn cascade(pairs: &mut [Svf], input: f32, f: f32, q: f32, kind: FilterKind) -> f32 {
    pairs
        .iter_mut()
        .fold(input, |signal, pair| pair.step(signal, f, q, kind))
}

/// Filter `buf` in place, taking the cutoff for sample `i` from `cutoffs[i]` (the
/// caller bakes the envelope and LFO into that track). `sample_rate` is the output
/// rate; the filter itself runs at twice it.
pub(crate) fn apply(buf: &mut [f32], filter: &Filter, cutoffs: &[f32], sample_rate: f32) {
    let mut svf = [Svf::default(); MAX_PAIRS];
    let pairs = &mut svf[..filter.slope.pole_pairs()];
    let q = (1.0 - filter.resonance.clamp(0.0, 0.99)).clamp(0.05, 1.0);
    let ceiling = sample_rate * MAX_CUTOFF_RATIO;
    // The two input samples before this one, as they were before filtering.
    let (mut before, mut last) = (0.0, 0.0);
    for i in 0..buf.len() {
        let current = buf[i];
        // Past the end, the parabola through the last three samples carries on:
        // a zero there would be a step, and a step is broadband.
        let next = buf
            .get(i + 1)
            .copied()
            .unwrap_or(3.0 * (current - last) + before);
        let cutoff = cutoffs.get(i).copied().unwrap_or(filter.cutoff);
        let f = tuning(cutoff.clamp(MIN_CUTOFF, ceiling), sample_rate);
        // Two half-rate steps per output sample (the 2× oversampling): the first
        // takes the point halfway from the last sample to this one, the second
        // this one, and its output is the sample we keep.
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

/// The point halfway between `b` and `c`, by the cubic through all four samples
/// — the half-band interpolator `[-1, 9, 9, -1] / 16`.
///
/// This is what keeps a highpass's stopband the filter's own. Running at 2× puts
/// an image of every tone `f` at `sr − f`; a highpass passes that image, and
/// keeping every other output folds it straight back onto `f`. Holding the
/// sample (feeding it twice) leaves the image ~`f / sr` down, a 6 dB-per-octave
/// floor under any slope; a straight line between `b` and `c` gets it to
/// ~`(f / sr)²`, still −59 dB for 560 Hz under a 24 dB highpass at 9 kHz; the
/// cubic to ~`(f / sr)⁴`, under the slope itself (−95 dB there, the slope −96).
/// Averaging the two half-step outputs would also clear the image, but darkens
/// every open lowpass (−3.7 dB at 15 kHz); this leaves the kept samples alone.
#[inline]
fn halfway(a: f32, b: f32, c: f32, d: f32) -> f32 {
    (9.0 * (b + c) - (a + d)) / 16.0
}

/// The SVF tuning coefficient for a cutoff, at 2× oversampling: `2·sin(π·fc/2·sr)`.
#[inline]
fn tuning(cutoff: f32, sample_rate: f32) -> f32 {
    (2.0 * (PI * cutoff / (2.0 * sample_rate)).sin()).min(1.0)
}

#[cfg(test)]
mod tests;
