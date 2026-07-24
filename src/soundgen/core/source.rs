//! src/soundgen/core/source.rs — the source stage: which generator makes the tone
//! (#357).
//!
//! One `match` over [`Source`], the head of the fixed signal path. Each arm hands
//! off to the module that owns that algorithm; nothing here knows any DSP. Sources
//! that can follow a moving pitch take the per-sample frequency track; Karplus-Strong
//! takes only the starting frequency, because its delay-line length is fixed once the
//! string is plucked.

use super::{fm, karplus, noise, osc};
use crate::soundgen::patch::Source;

/// Render `source` into `out` (which the caller sized and zeroed), following the
/// per-sample frequency track `freqs`.
pub fn render(source: &Source, freqs: &[f32], seed: u64, out: &mut [f32], rate: f32) {
    match source {
        Source::OscStack { oscs } => osc::render(oscs, freqs, out, rate),
        Source::Karplus {
            damping,
            brightness,
        } => {
            let freq = freqs.first().copied().unwrap_or(0.0);
            karplus::render(out, freq, *damping, *brightness, seed, rate);
        }
        Source::Noise => noise::fill(out, seed),
        Source::Fm2 {
            ratio,
            index,
            mod_decay,
        } => fm::render(out, freqs, *ratio, *index, *mod_decay, rate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soundgen::patch::{Osc, Wave};

    fn render_kind(source: &Source) -> Vec<f32> {
        let mut out = vec![0.0; 8192];
        render(source, &vec![220.0; 8192], 5, &mut out, 44_100.0);
        out
    }

    #[test]
    fn every_source_kind_produces_audible_finite_samples() {
        let kinds = [
            Source::OscStack {
                oscs: vec![Osc {
                    wave: Wave::Saw,
                    detune_cents: 0.0,
                    gain: 1.0,
                    octave: 0,
                }],
            },
            Source::Karplus {
                damping: 0.99,
                brightness: 0.5,
            },
            Source::Noise,
            Source::Fm2 {
                ratio: 2.0,
                index: 4.0,
                mod_decay: 0.3,
            },
        ];
        for source in kinds {
            let buf = render_kind(&source);
            assert!(
                buf.iter().all(|s| s.is_finite()),
                "{source:?} went non-finite"
            );
            let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak > 0.2, "{source:?} is inaudible (peak {peak})");
            assert!(peak <= 1.0 + 1e-3, "{source:?} peaked at {peak}");
        }
    }

    #[test]
    fn an_empty_frequency_track_does_not_panic() {
        let mut out = vec![];
        for source in [
            Source::Noise,
            Source::Karplus {
                damping: 0.9,
                brightness: 0.5,
            },
        ] {
            render(&source, &[], 1, &mut out, 44_100.0);
        }
        assert!(out.is_empty());
    }
}
