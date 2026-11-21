//! Filter bands, from the RBJ cookbook.

use crate::level::db_to_gain_f32;
use crate::patch::{EqBand, EqKind};

const MIN_Q: f32 = 0.1;

const MAX_Q: f32 = 30.0;

const MAX_GAIN_DB: f32 = 24.0;

const MIN_HZ: f32 = 10.0;

const NYQUIST_MARGIN: f32 = 0.45;

pub(crate) fn apply(buf: &mut [f32], bands: &[EqBand], rate: f32) {
    for band in bands {
        if let Some(biquad) = Biquad::of(band, rate) {
            biquad.run(buf);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    fn of(band: &EqBand, rate: f32) -> Option<Self> {
        if !rate.is_finite() || rate <= 0.0 {
            return None;
        }
        let gain_db = finite(band.gain_db, 0.0).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
        if band.kind.takes_gain() && gain_db == 0.0 {
            return None;
        }
        let ceiling = (rate * NYQUIST_MARGIN).max(MIN_HZ);
        let hz = finite(band.hz(), band.kind.crossover()).clamp(MIN_HZ, ceiling);
        let q = finite(band.q, 0.707).clamp(MIN_Q, MAX_Q);

        let w0 = std::f32::consts::TAU * hz / rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        // The cookbook's `A`: an amplitude ratio at *half* the gain, because a bell and a shelf
        // each apply it twice over.
        let a = db_to_gain_f32(gain_db / 2.0);

        let (b, denominator) = match band.kind {
            EqKind::HighPass => (
                [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
                [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
            ),
            EqKind::LowPass => (
                [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
                [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
            ),
            EqKind::Peak => (
                [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
                [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
            ),
            EqKind::LowShelf => {
                let shared = 2.0 * a.sqrt() * alpha;
                (
                    [
                        a * ((a + 1.0) - (a - 1.0) * cos + shared),
                        2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                        a * ((a + 1.0) - (a - 1.0) * cos - shared),
                    ],
                    [
                        (a + 1.0) + (a - 1.0) * cos + shared,
                        -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                        (a + 1.0) + (a - 1.0) * cos - shared,
                    ],
                )
            }
            EqKind::HighShelf => {
                let shared = 2.0 * a.sqrt() * alpha;
                (
                    [
                        a * ((a + 1.0) + (a - 1.0) * cos + shared),
                        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                        a * ((a + 1.0) + (a - 1.0) * cos - shared),
                    ],
                    [
                        (a + 1.0) - (a - 1.0) * cos + shared,
                        2.0 * ((a - 1.0) - (a + 1.0) * cos),
                        (a + 1.0) - (a - 1.0) * cos - shared,
                    ],
                )
            }
        };

        let a0 = denominator[0];
        let coefficients = Self {
            b0: b[0] / a0,
            b1: b[1] / a0,
            b2: b[2] / a0,
            a1: denominator[1] / a0,
            a2: denominator[2] / a0,
        };
        coefficients.is_finite().then_some(coefficients)
    }

    fn is_finite(&self) -> bool {
        [self.b0, self.b1, self.b2, self.a1, self.a2]
            .iter()
            .all(|c| c.is_finite())
    }

    fn run(self, buf: &mut [f32]) {
        let (mut z1, mut z2) = (0.0f32, 0.0f32);
        for sample in buf.iter_mut() {
            let x = if sample.is_finite() { *sample } else { 0.0 };
            let y = self.b0 * x + z1;
            z1 = self.b1 * x - self.a1 * y + z2;
            z2 = self.b2 * x - self.a2 * y;
            *sample = y;
        }
    }
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::gain_to_db;
    use std::f32::consts::TAU;

    const RATE: f32 = 44_100.0;
    const N: usize = 11_025;

    fn sine(freq: f32) -> Vec<f32> {
        (0..N)
            .map(|i| (TAU * freq * i as f32 / RATE).sin())
            .collect()
    }

    fn band(kind: EqKind, freq: f32, gain_db: f32, q: f32) -> EqBand {
        EqBand {
            kind,
            freq: Some(freq),
            gain_db,
            q,
        }
    }

    fn magnitude_at(buf: &[f32], freq: f32) -> f64 {
        let settled = &buf[N / 2..];
        let w = std::f64::consts::TAU * f64::from(freq) / f64::from(RATE);
        let (re, im) = settled
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(re, im), (i, s)| {
                let (phase, s) = (w * i as f64, f64::from(*s));
                (re + s * phase.cos(), im + s * phase.sin())
            });
        2.0 * re.hypot(im) / settled.len() as f64
    }

    fn response_db(band: EqBand, probe: f32) -> f64 {
        let mut buf = sine(probe);
        apply(&mut buf, &[band], RATE);
        gain_to_db(magnitude_at(&buf, probe))
    }

    #[test]
    fn a_cut_removes_the_decibels_it_asked_for_and_leaves_the_rest_alone() {
        // The move the issue is about: 250 Hz out of the pad, the pad kept.
        let cut = band(EqKind::Peak, 250.0, -9.0, 2.0);
        assert!(
            (response_db(cut, 250.0) + 9.0).abs() < 0.5,
            "at the centre: {}",
            response_db(cut, 250.0)
        );
        assert!(
            response_db(cut, 4000.0).abs() < 0.5,
            "a decade and a half away, nothing moved: {}",
            response_db(cut, 4000.0)
        );
    }

    #[test]
    fn a_boost_is_the_same_curve_the_other_way_up() {
        let boost = band(EqKind::Peak, 1000.0, 6.0, 1.0);
        assert!((response_db(boost, 1000.0) - 6.0).abs() < 0.5);
    }

    #[test]
    fn the_pass_filters_remove_the_end_they_are_named_for() {
        let high_pass = band(EqKind::HighPass, 250.0, 0.0, 0.707);
        assert!(
            response_db(high_pass, 60.0) < -18.0,
            "two octaves under a 12 dB/oct corner: {}",
            response_db(high_pass, 60.0)
        );
        assert!(
            response_db(high_pass, 2000.0).abs() < 0.5,
            "and passes above"
        );

        let low_pass = band(EqKind::LowPass, 4000.0, 0.0, 0.707);
        assert!(response_db(low_pass, 16_000.0) < -18.0);
        assert!(response_db(low_pass, 500.0).abs() < 0.5);
    }

    #[test]
    fn a_shelf_moves_a_whole_end_by_its_gain() {
        let low = band(EqKind::LowShelf, 250.0, -6.0, 0.707);
        assert!(
            (response_db(low, 40.0) + 6.0).abs() < 0.6,
            "well below the corner it is the full cut: {}",
            response_db(low, 40.0)
        );
        assert!(response_db(low, 4000.0).abs() < 0.3, "and flat above it");

        let air = band(EqKind::HighShelf, 4000.0, 4.0, 0.707);
        assert!((response_db(air, 16_000.0) - 4.0).abs() < 0.6);
        assert!(response_db(air, 200.0).abs() < 0.3);
    }

    #[test]
    fn a_gain_of_zero_is_not_an_approximation_of_the_input() {
        // The property that lets a recipe park a band it is still thinking about: identical
        // samples, not almost-identical ones.
        for kind in [EqKind::LowShelf, EqKind::Peak, EqKind::HighShelf] {
            let original = sine(440.0);
            let mut parked = original.clone();
            apply(&mut parked, &[band(kind, 250.0, 0.0, 1.4)], RATE);
            assert_eq!(parked, original, "{kind:?} at 0 dB must be a bypass");
        }
    }

    #[test]
    fn every_kind_that_reads_a_gain_moves_its_own_band_by_it() {
        for (kind, corner, probe) in [
            (EqKind::LowShelf, 250.0, 40.0),
            (EqKind::Peak, 250.0, 250.0),
            (EqKind::HighShelf, 4000.0, 16_000.0),
        ] {
            assert!(kind.takes_gain(), "{kind:?} is written with a gain");
            let flat = response_db(band(kind, corner, 0.0, 0.707), probe);
            let lifted = response_db(band(kind, corner, 6.0, 0.707), probe);
            assert!(flat.abs() < 0.4, "{kind:?} at 0 dB leaves it alone: {flat}");
            assert!(
                (lifted - flat - 6.0).abs() < 0.6,
                "{kind:?} at +6 dB lifts its own band by it: {flat} to {lifted}"
            );
        }
    }

    #[test]
    fn the_pass_filters_have_no_gain_to_read() {
        for (kind, corner, probe) in [
            (EqKind::HighPass, 250.0, 60.0),
            (EqKind::LowPass, 4000.0, 16_000.0),
        ] {
            assert!(!kind.takes_gain(), "{kind:?} has no amount of gone to ask");
            assert_eq!(
                response_db(band(kind, corner, 0.0, 0.707), probe),
                response_db(band(kind, corner, 6.0, 0.707), probe),
                "{kind:?} read a gain it does not have"
            );
        }
    }

    #[test]
    fn bands_stack_and_the_defaults_are_the_report_s_own_crossovers() {
        let written = [
            band(EqKind::HighPass, 250.0, 0.0, 0.707),
            band(EqKind::HighShelf, 4000.0, 4.0, 0.707),
        ];
        let defaulted = [
            EqBand {
                kind: EqKind::HighPass,
                freq: None,
                gain_db: 0.0,
                q: 0.707,
            },
            EqBand {
                kind: EqKind::HighShelf,
                freq: None,
                gain_db: 4.0,
                q: 0.707,
            },
        ];
        let mut spelled_out = sine(60.0);
        let mut left_to_default = sine(60.0);
        apply(&mut spelled_out, &written, RATE);
        apply(&mut left_to_default, &defaulted, RATE);
        assert_eq!(spelled_out, left_to_default);
        assert!(magnitude_at(&spelled_out, 60.0) < 0.2, "and both filtered");
    }

    #[test]
    fn a_degenerate_band_is_a_no_op_rather_than_a_buffer_of_nan() {
        let original = sine(440.0);
        let broken = [
            band(EqKind::Peak, f32::NAN, 6.0, 1.0),
            band(EqKind::LowPass, 1000.0, 0.0, f32::NAN),
            band(EqKind::HighPass, -50.0, 0.0, 0.0),
            band(EqKind::HighShelf, 1e9, 6.0, 1e9),
            band(EqKind::Peak, 1000.0, f32::INFINITY, 1.0),
        ];
        for band in broken {
            let mut buf = original.clone();
            apply(&mut buf, &[band], RATE);
            assert!(
                buf.iter().all(|s| s.is_finite()),
                "{band:?} poisoned the signal"
            );
        }
        let mut buf = original.clone();
        apply(&mut buf, &broken[..1], 0.0);
        assert_eq!(buf, original, "a rate of nothing filters nothing");
    }

    #[test]
    fn a_poisoned_sample_does_not_poison_the_rest_of_the_signal() {
        let mut buf = sine(440.0);
        buf[100] = f32::NAN;
        apply(&mut buf, &[band(EqKind::Peak, 250.0, -6.0, 2.0)], RATE);
        assert!(buf.iter().all(|s| s.is_finite()));
    }
}
