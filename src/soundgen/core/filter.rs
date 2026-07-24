//! src/soundgen/core/filter.rs — the Chamberlin state-variable filter (#357).
//!
//! The filter is what makes a subtractive patch expressive: a raw saw stack is a
//! buzz until an envelope sweeps a resonant cutoff across it. We use the
//! **Chamberlin SVF** — two integrators in a loop yielding lowpass, highpass and
//! bandpass from one topology, with an honest resonance knob — because it is stable
//! at audio rates, costs a handful of adds per sample, and takes a *new cutoff every
//! sample* for free (which is exactly what an envelope sweep and an LFO wobble need).
//! Only lowpass and highpass are exposed in v1.
//!
//! The classic caveat is that the plain form goes unstable as the cutoff approaches
//! a quarter of the sample rate. We run it **2× oversampled** (two half-steps per
//! output sample), which pushes the stable ceiling up to ~`sr/3`, and clamp the
//! cutoff there — so a "fully open" filter really is open across the audible band.

use std::f32::consts::PI;

use crate::soundgen::patch::{Filter, FilterKind};

/// Lowest cutoff a track is clamped to, in Hz — below hearing, and far from the
/// zero that would stall the integrators.
const MIN_CUTOFF: f32 = 20.0;
/// Highest cutoff as a fraction of the sample rate — the 2×-oversampled SVF's
/// stability ceiling.
const MAX_CUTOFF_RATIO: f32 = 1.0 / 3.0;

/// Two integrator states — the whole filter memory.
#[derive(Default)]
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
        }
    }
}

/// Filter `buf` in place, taking the cutoff for sample `i` from `cutoffs[i]` (the
/// caller bakes the envelope and LFO into that track). `sample_rate` is the output
/// rate; the filter itself runs at twice it.
pub fn apply(buf: &mut [f32], filter: &Filter, cutoffs: &[f32], sample_rate: f32) {
    let mut svf = Svf::default();
    let q = (1.0 - filter.resonance.clamp(0.0, 0.99)).clamp(0.05, 1.0);
    let ceiling = sample_rate * MAX_CUTOFF_RATIO;
    for (i, s) in buf.iter_mut().enumerate() {
        let cutoff = cutoffs.get(i).copied().unwrap_or(filter.cutoff);
        let f = tuning(cutoff.clamp(MIN_CUTOFF, ceiling), sample_rate);
        // Two half-rate steps per output sample (the 2× oversampling); the second
        // one's output is the sample we keep.
        svf.step(*s, f, q, filter.kind);
        *s = svf.step(*s, f, q, filter.kind);
    }
}

/// The SVF tuning coefficient for a cutoff, at 2× oversampling: `2·sin(π·fc/2·sr)`.
#[inline]
fn tuning(cutoff: f32, sample_rate: f32) -> f32 {
    (2.0 * (PI * cutoff / (2.0 * sample_rate)).sin()).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soundgen::patch::Adsr;
    use std::f32::consts::TAU;

    fn filter(kind: FilterKind, cutoff: f32) -> Filter {
        Filter {
            kind,
            cutoff,
            resonance: 0.0,
            env_amount: 0.0,
            adsr: Adsr::default(),
        }
    }

    /// A sine at `hz`, one second long at 44.1 kHz.
    fn sine(hz: f32) -> Vec<f32> {
        (0..44_100)
            .map(|i| (TAU * hz * i as f32 / 44_100.0).sin())
            .collect()
    }

    /// Peak amplitude over the settled second half of a buffer.
    fn settled_peak(buf: &[f32]) -> f32 {
        buf[buf.len() / 2..]
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()))
    }

    #[test]
    fn lowpass_passes_below_and_stops_above_the_cutoff() {
        let cutoffs = vec![1000.0; 44_100];
        let mut low = sine(100.0);
        let mut high = sine(8000.0);
        apply(
            &mut low,
            &filter(FilterKind::Lowpass, 1000.0),
            &cutoffs,
            44_100.0,
        );
        apply(
            &mut high,
            &filter(FilterKind::Lowpass, 1000.0),
            &cutoffs,
            44_100.0,
        );
        assert!(settled_peak(&low) > 0.7, "100 Hz survives a 1 kHz lowpass");
        assert!(settled_peak(&high) < 0.1, "8 kHz does not");
    }

    #[test]
    fn highpass_is_the_mirror_image() {
        let cutoffs = vec![2000.0; 44_100];
        let mut low = sine(100.0);
        let mut high = sine(8000.0);
        apply(
            &mut low,
            &filter(FilterKind::Highpass, 2000.0),
            &cutoffs,
            44_100.0,
        );
        apply(
            &mut high,
            &filter(FilterKind::Highpass, 2000.0),
            &cutoffs,
            44_100.0,
        );
        assert!(settled_peak(&low) < 0.1, "100 Hz is cut");
        assert!(settled_peak(&high) > 0.7, "8 kHz passes");
    }

    #[test]
    fn resonance_lifts_the_response_at_the_cutoff() {
        let cutoffs = vec![1000.0; 44_100];
        let mut flat = sine(1000.0);
        let mut resonant = sine(1000.0);
        let mut f = filter(FilterKind::Lowpass, 1000.0);
        apply(&mut flat, &f, &cutoffs, 44_100.0);
        f.resonance = 0.9;
        apply(&mut resonant, &f, &cutoffs, 44_100.0);
        assert!(settled_peak(&resonant) > settled_peak(&flat));
    }

    #[test]
    fn extreme_cutoffs_stay_finite_and_bounded() {
        for cutoff in [0.0, -500.0, 1e9, f32::INFINITY] {
            let mut buf = sine(440.0);
            let cutoffs = vec![cutoff; buf.len()];
            let mut f = filter(FilterKind::Lowpass, 1000.0);
            f.resonance = 0.99;
            apply(&mut buf, &f, &cutoffs, 44_100.0);
            assert!(
                buf.iter().all(|s| s.is_finite()),
                "cutoff {cutoff} produced non-finite samples"
            );
        }
    }

    #[test]
    fn a_poisoned_sample_resets_the_filter_instead_of_spreading() {
        // One non-finite input would otherwise infect the integrators forever, so
        // every later sample would come out NaN. The guard flushes the state.
        let mut buf = sine(440.0);
        buf[100] = f32::INFINITY;
        buf[200] = f32::NAN;
        let cutoffs = vec![1000.0; buf.len()];
        apply(
            &mut buf,
            &filter(FilterKind::Lowpass, 1000.0),
            &cutoffs,
            44_100.0,
        );
        assert!(
            buf.iter().all(|s| s.is_finite()),
            "the poison was contained"
        );
        // And the filter is still a filter afterwards, not a stuck zero.
        assert!(
            settled_peak(&buf) > 0.5,
            "it recovered and kept passing signal"
        );
    }
}
