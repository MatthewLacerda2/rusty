//! pulse: a square at any duty cycle.
//!
//! A square is a pulse that happens to be high for half its cycle. Narrowed,
//! it is the other two chiptune duties (25% and 12.5%) and the nasal reed and
//! clav tones; kept moving by an LFO, it is pulse-width modulation, the warm
//! single-oscillator pad.
//!
//! Two things change when the width leaves one half, and this module owns
//! both. The step down no longer sits half a cycle from the step up, so each
//! edge takes its own polyBLEP correction at its own place. And the pulse is
//! no longer centred on zero: high for `w` of the cycle and low for the rest,
//! it averages `2w - 1`, a constant offset that a filter would carry and a
//! limiter would spend headroom on. It is subtracted, sample by sample, so a
//! moving width does not leave its own slow swell in the signal either.
//!
//! At one half both corrections are exactly the square's — the fall is
//! `phase + 0.5` and the offset is `0.0` — so a square nobody narrowed renders
//! the samples it always did, bit for bit.

use super::poly_blep;
use crate::core::lfo::Lfos;
use crate::patch::{LfoTarget, MAX_PULSE_WIDTH, MIN_PULSE_WIDTH, Osc};

/// How far the LFOs aimed at `pulse_width` swing every square's width at
/// each of `n` samples, `depth` either way and two of them added — or
/// nothing, which leaves each width as written. Clamping the sum to the range
/// is [`width`]'s.
///
/// Beside the pulse rather than with the other LFO readers in
/// [`crate::core::tracks`], because that file is at the size gate.
pub(crate) fn track(lfos: &Lfos, n: usize) -> Option<Vec<f32>> {
    if !lfos.aims_at(LfoTarget::PulseWidth) {
        return None;
    }
    Some(
        (0..n)
            .map(|i| lfos.offset(LfoTarget::PulseWidth, i))
            .collect(),
    )
}

/// The width oscillator `osc` is drawn at, `offset` (an LFO's swing) added to
/// its written one, clamped to the range. A width or an offset that is not a
/// number is the square's half and no swing.
pub(super) fn width(osc: &Osc, offset: f32) -> f32 {
    let written = if osc.pulse_width.is_finite() {
        osc.pulse_width
    } else {
        0.5
    };
    let offset = if offset.is_finite() { offset } else { 0.0 };
    (written + offset).clamp(MIN_PULSE_WIDTH, MAX_PULSE_WIDTH)
}

/// One sample of a band-limited, DC-free pulse at normalized `phase`, high
/// for the first `width` of the cycle; `dt` sizes the polyBLEP window.
pub(super) fn sample(phase: f32, dt: f32, width: f32) -> f32 {
    let naive = if phase < width { 1.0 } else { -1.0 };
    // `1.0 - width` first, so at one half the fall is `phase + 0.5` exactly,
    // rounded the way the square always rounded it.
    let fall = (phase + (1.0 - width)).fract();
    naive + poly_blep(phase, dt) - poly_blep(fall, dt) - (2.0 * width - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Lfo, Wave};

    /// 441 Hz at 44.1 kHz: exactly a hundred samples a cycle, so a cycle's
    /// mean and its time high are read off whole samples.
    const DT: f32 = 0.01;

    /// One cycle, sampled half a sample off the grid so no sample lands on
    /// an edge, where which side it rounds to would be the test's own error.
    fn cycle(width: f32) -> Vec<f32> {
        (0..100)
            .map(|n| sample((n as f32 + 0.5) * DT, DT, width))
            .collect()
    }

    fn square(pulse_width: f32) -> Osc {
        Osc {
            wave: Wave::Square,
            detune_cents: 0.0,
            gain: 1.0,
            octave: 0,
            voices: 1,
            spread: 12.0,
            width: 0.0,
            pulse_width,
        }
    }

    /// The promise that keeps `SYNTH_VERSION` where it is: at one half this is
    /// the square as it was written before the width existed, bit for bit.
    #[test]
    fn half_width_is_the_square_it_always_was() {
        for n in 0..4096 {
            let phase = (n as f32 * 0.000_731).fract();
            for dt in [0.001, 0.01, 0.11, 0.5] {
                let was = if phase < 0.5 { 1.0 } else { -1.0 } + poly_blep(phase, dt)
                    - poly_blep((phase + 0.5).fract(), dt);
                assert_eq!(sample(phase, dt, 0.5).to_bits(), was.to_bits());
            }
        }
    }

    /// The DC is removed at every width, so narrowing a pulse never pushes a
    /// constant offset into the filter and the limiter.
    #[test]
    fn a_cycle_averages_zero_at_any_width() {
        for width in [0.05, 0.125, 0.25, 0.5, 0.7, 0.95] {
            let mean = cycle(width).iter().sum::<f32>() / 100.0;
            assert!(mean.abs() < 1e-4, "width {width} averages {mean}");
        }
    }

    /// The width is the share of the cycle spent high.
    #[test]
    fn the_high_part_lasts_the_width_of_the_cycle() {
        for width in [0.125, 0.25, 0.5, 0.75] {
            let high = cycle(width).iter().filter(|s| **s > 0.0).count();
            let want = (width * 100.0).round() as usize;
            assert!(high.abs_diff(want) <= 1, "width {width} high for {high}");
        }
    }

    /// Both edges are corrected: a narrow pulse at 5 kHz (dt ≈ 0.11) does not
    /// swing the naive 2.0 between neighbouring samples at either of them.
    #[test]
    fn band_limiting_tames_a_high_narrow_pulse() {
        let dt = 5000.0 / 44_100.0;
        let buf: Vec<f32> = (0..4410)
            .map(|n| sample((n as f32 * dt).fract(), dt, 0.25))
            .collect();
        let worst = buf
            .windows(2)
            .fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(worst < 1.6, "the pulse still steps by {worst}");
    }

    /// A pulse-width LFO swings by its depth, either way — at 5 Hz the peak
    /// is a quarter cycle in, sample 2205 — and nothing else moves a width.
    #[test]
    fn a_pulse_width_lfo_swings_by_its_depth() {
        let lfo = |target| Lfos::of([&Lfo::new(5.0, 0.3, target)], 0, None, 8820);
        let swing = track(&lfo(LfoTarget::PulseWidth), 8820).unwrap();
        assert_eq!((swing.len(), swing[0]), (8820, 0.0));
        assert!((swing[2205] - 0.3).abs() < 1e-6, "peak {}", swing[2205]);
        assert!((swing[6615] + 0.3).abs() < 1e-6, "trough {}", swing[6615]);
        for target in [LfoTarget::Pitch, LfoTarget::Cutoff, LfoTarget::Amp] {
            assert_eq!(track(&lfo(target), 8820), None);
        }
        assert_eq!(track(&Lfos::of([], 0, None, 8820), 8820), None);
    }

    /// Clamped at both ends, an LFO's swing included, and nonsense is the
    /// plain square.
    #[test]
    fn the_width_is_clamped_to_the_range() {
        assert_eq!(width(&square(0.25), 0.0), 0.25);
        assert_eq!(width(&square(0.25), 0.1), 0.35);
        assert_eq!(width(&square(0.0), 0.0), MIN_PULSE_WIDTH);
        assert_eq!(width(&square(0.5), 0.9), MAX_PULSE_WIDTH);
        assert_eq!(width(&square(0.5), -0.9), MIN_PULSE_WIDTH);
        assert_eq!(width(&square(f32::NAN), f32::INFINITY), 0.5);
    }
}
