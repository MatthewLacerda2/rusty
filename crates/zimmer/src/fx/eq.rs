//! Filter bands, from the RBJ cookbook.
//!
//! Robert Bristow-Johnson's **Audio EQ Cookbook** is the public-domain page
//! every equaliser at this scale is built from: closed-form second-order
//! coefficients for a low-pass, a high-pass, a peaking bell and a shelf at
//! either end, derived from an analog prototype and bilinear-transformed. Six
//! lines of arithmetic per band, no tuning tables to invent, and the same
//! formulas are inside more or less every plugin anyone has ever mixed
//! through — the "standard, proven algorithm" bar this layer is held to, the
//! way [`super::reverb`] is held to Freeverb.
//!
//! ## Why a biquad and not the SVF already in the crate
//!
//! [`crate::core::filter`] is a Chamberlin state-variable filter, and it is
//! there for a property this does not need: it takes a **new cutoff every
//! sample** for free, which is what an envelope sweep and an LFO wobble are
//! made of. It pays for that with a response that is only approximately what
//! its cutoff says, and it has no shelving or peaking form at all.
//!
//! An EQ band is the opposite case. It is *static* — set once for the whole
//! signal — so the coefficients are computed once and the per-sample cost is
//! five multiplies. What matters instead is that the curve is exactly the one
//! asked for: a flat pass band, a cut of exactly the decibels written, and no
//! surprise resonance at the corner. That is the biquad's whole reason to
//! exist, and it is why a mixing move belongs in one and an instrument's
//! filter sweep does not.
//!
//! ## Zero gain is a bypass, and it is exact
//!
//! A peaking band at `0.0` dB is mathematically the identity, but running the
//! difference equation for it is not: the coefficients round, and a signal
//! comes back a few ulps away from where it went in. So a band whose kind
//! reads a gain and whose gain is zero is **skipped entirely** rather than
//! computed. That is the difference between a recipe that can list its bands
//! and sweep one of them, and a recipe where every parked band quietly
//! colours the mix.
//!
//! ## No tail
//!
//! [`super::tail_seconds`] gets nothing from an EQ, the way it gets nothing
//! from [`super::saturate`]. A biquad does have memory, so this is a claim
//! rather than a definition — but its ring-down is bounded by roughly
//! `Q / (π · f)`, which at the extremes this clamps to ([`MAX_Q`] at
//! [`MIN_HZ`]) is a few hundred milliseconds and at any band anyone would
//! write is under ten. More to the point it decays *with* the signal rather
//! than after it: an EQ neither delays nor repeats anything, so there is no
//! echo to cut off mid-repeat, which is what the tail exists to protect.
//! Padding every note for it would grow every recipe using an EQ for nothing.

use crate::level::db_to_gain_f32;
use crate::patch::{EqBand, EqKind};

/// The widest a band may be. Below this a "band" is most of the spectrum and
/// the shelves stop having a corner at all.
const MIN_Q: f32 = 0.1;

/// The narrowest a band may be. A `Q` of 30 at 250 Hz is 8 Hz wide — past
/// surgical and into ringing on its own, which is a resonator rather than an
/// equaliser.
const MAX_Q: f32 = 30.0;

/// The most a band may move, in decibels either way. 24 dB is a region
/// removed or a region tripled; anything past it is not mixing.
const MAX_GAIN_DB: f32 = 24.0;

/// The lowest a band may sit. Under this is below what anything reproduces,
/// and a corner at 0 Hz has no `w0` to speak of.
const MIN_HZ: f32 = 10.0;

/// How far below Nyquist the highest band may sit, as a fraction of the rate.
///
/// The bilinear transform warps the frequency axis, and a corner pushed
/// against Nyquist warps into a shape that is no longer the one asked for. At
/// 44.1 kHz this puts the ceiling near 19.8 kHz, which is above anything a
/// mix decision is made about.
const NYQUIST_MARGIN: f32 = 0.45;

/// Filter `buf` in place through each band in turn.
///
/// Every band is computed once and then run over the whole signal, so the cost
/// is linear in bands and samples both. A band that would do nothing — a zero
/// gain, a nonsense frequency — is skipped rather than run at unity, which is
/// what makes the bypass exact.
pub(crate) fn apply(buf: &mut [f32], bands: &[EqBand], rate: f32) {
    for band in bands {
        if let Some(biquad) = Biquad::of(band, rate) {
            biquad.run(buf);
        }
    }
}

/// One band's coefficients, normalised so `a0` is 1.
#[derive(Clone, Copy, Debug)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Biquad {
    /// The cookbook coefficients for `band`, or `None` when the band is a
    /// bypass.
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
        // The cookbook's `A`: an amplitude ratio at *half* the gain, because a
        // bell and a shelf each apply it twice over.
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
        // A degenerate `a0` would hand the signal a buffer of NaN rather than
        // an equalised one, and a report of a silent mix is a long way from
        // the field that caused it.
        coefficients.is_finite().then_some(coefficients)
    }

    fn is_finite(&self) -> bool {
        [self.b0, self.b1, self.b2, self.a1, self.a2]
            .iter()
            .all(|c| c.is_finite())
    }

    /// Run the band over the whole signal, transposed direct form II.
    ///
    /// That form rather than direct form I because it carries two state
    /// variables instead of four and is the better-conditioned of the two in
    /// `f32` — the difference shows up exactly where an EQ is most used, on a
    /// low-frequency band where the poles crowd together.
    fn run(self, buf: &mut [f32]) {
        let (mut z1, mut z2) = (0.0f32, 0.0f32);
        for sample in buf.iter_mut() {
            // A non-finite sample would poison the state for the rest of the
            // signal rather than for itself, so it is dropped — the same guard
            // the patch filter and the band meter keep.
            let x = if sample.is_finite() { *sample } else { 0.0 };
            let y = self.b0 * x + z1;
            z1 = self.b1 * x - self.a1 * y + z2;
            z2 = self.b2 * x - self.a2 * y;
            *sample = y;
        }
    }
}

/// `value` if it is a number at all, and `fallback` if it is not.
fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::gain_to_db;
    use std::f32::consts::TAU;

    const RATE: f32 = 44_100.0;
    /// A quarter-second, long enough for every band here to settle well before
    /// the window the magnitude is read over.
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

    /// Amplitude at `freq`, over the settled second half of the buffer so the
    /// filter's own start-up transient is not measured as signal.
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

    /// What one band does to a tone at `probe`, in decibels.
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
        // The property that lets a recipe park a band it is still thinking
        // about: identical samples, not almost-identical ones.
        for kind in [EqKind::LowShelf, EqKind::Peak, EqKind::HighShelf] {
            let original = sine(440.0);
            let mut parked = original.clone();
            apply(&mut parked, &[band(kind, 250.0, 0.0, 1.4)], RATE);
            assert_eq!(parked, original, "{kind:?} at 0 dB must be a bypass");
        }
    }

    /// Which kinds read a `gain_db`, asserted on [`EqKind::takes_gain`]
    /// directly — because the classification is the contract and no
    /// measurement can see it move (#485).
    ///
    /// The function gates the zero-gain bypass and nothing else: the gain
    /// reaches the coefficients whatever it says, and the cookbook's `A` at
    /// 0 dB is exactly 1, which makes a parked band's biquad an exact identity
    /// rather than an approximate one. So `takes_gain` returning `false` for
    /// all five kinds costs a pass over the buffer and moves no sample — which
    /// is why the bypass can promise bit-equality in the first place, and why
    /// pinning it means saying which kinds are in the set. The same three are
    /// what `docs/recipes.md` promises whoever writes the band.
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

    /// The mirror, and the half of the rule a bare `false` would also satisfy:
    /// `gain_db` is not a field these two read, so writing one changes nothing
    /// — not within rounding, but the same samples. That a zero gain does not
    /// switch them off, the way it switches off the three above, is what the
    /// filtering test over these same two bands already says.
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
