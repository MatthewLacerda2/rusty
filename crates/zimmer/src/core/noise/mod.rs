//! the one source of randomness in a bake.
//!
//! Every stochastic element in the synth — the `noise` source, the Karplus-Strong
//! excitation — draws from here, and here draws from the **seeded integer hash**
//! `procgen` established ([`crate::hash`]). No `rand`, no wall clock: the
//! sample at index `i` is a pure function of `(i, channel, seed)`, which is what
//! makes "same patch + note + seed ⇒ byte-identical WAV" true rather than hoped for.
//!
//! What the draw is *shaped* into is [`color`]'s business: white is the draw
//! itself, and pink and brown are that draw through a filter whose slope is
//! the colour. That module owns the measurements and the reasoning; this one
//! owns the draw and the two channels.

pub(crate) mod color;

use crate::hash::unit2;
use crate::patch::NoiseColor;
use crate::stereo::Stereo;
use color::Coloring;

/// The hash channel the left side of a noise source draws on.
///
/// Zero, which is what a mono bake always drew from — so the left channel of a
/// noise source is the signal it has always been, and only the right one is
/// new.
const LEFT_CHANNEL: u64 = 0;
/// The hash channel the right side draws on. Its own, so the two sides are
/// **uncorrelated**, which is the entire point: two independent noise draws
/// are as wide as a signal can be, and they cost nothing.
const RIGHT_CHANNEL: u64 = 0x4e32; // "N2"

/// One white-noise sample in `−1..1` for position `i` on `channel`, under `seed`.
/// `channel` keeps two consumers (say the excitation and a noise layer) from
/// drawing the identical sequence.
///
/// `i` is signed so a coloured filter can be primed on the noise the source
/// was already making *before* the note — see [`color::WARMUP`]. Every
/// non-negative index draws exactly what it always drew.
#[inline]
pub(crate) fn white(i: i64, channel: u64, seed: u64) -> f32 {
    unit2(i, 0, channel, seed) * 2.0 - 1.0
}

/// Fill `out` with noise of `color` — the `noise` source, unshaped (the filter
/// and amp envelope downstream are what turn it into a gunshot or a footstep).
///
/// **The one source that is stereo at the source.** Every other generator here
/// makes a single waveform that reaches both channels identically, and width
/// is added downstream by the mix; noise is the exception because a second
/// independent draw is free and is *more* faithful to what noise is than a
/// copy of the first would be. A hiss identical in both ears is a point in
/// space; two draws are a wall of it.
///
/// **Colour does not take that back.** Each side is drawn independently *and*
/// coloured independently, by its own filter over its own draw, so pink rain
/// and brown thunder are as wide as the white hiss was. Colouring one draw and
/// copying it would have been half the arithmetic and a mono source; see
/// [`Coloring`].
pub(crate) fn fill(out: &mut Stereo, color: NoiseColor, seed: u64) {
    color_into(&mut out.l, LEFT_CHANNEL, color, seed);
    color_into(&mut out.r, RIGHT_CHANNEL, color, seed);
}

/// One channel of it: prime the colour's filter on the draws before index
/// zero, then run the note itself through the same filter.
fn color_into(out: &mut [f32], channel: u64, color: NoiseColor, seed: u64) {
    let mut coloring = Coloring::new(color);
    for i in -(coloring.warmup() as i64)..0 {
        coloring.step(white(i, channel, seed));
    }
    for (i, sample) in out.iter_mut().enumerate() {
        *sample = coloring.step(white(i as i64, channel, seed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::gain_to_db_f32;

    fn filled(n: usize, color: NoiseColor, seed: u64) -> Stereo {
        let mut buf = Stereo::silence(n);
        fill(&mut buf, color, seed);
        buf
    }

    fn rms(channel: &[f32]) -> f32 {
        (channel.iter().map(|s| s * s).sum::<f32>() / channel.len() as f32).sqrt()
    }

    const EVERY_COLOR: [NoiseColor; 3] = [NoiseColor::White, NoiseColor::Pink, NoiseColor::Brown];

    #[test]
    fn noise_is_bounded_and_centred() {
        let buf = filled(8192, NoiseColor::White, 7);
        for channel in [&buf.l, &buf.r] {
            assert!(channel.iter().all(|s| (-1.0..=1.0).contains(s)));
            let mean: f32 = channel.iter().sum::<f32>() / channel.len() as f32;
            assert!(mean.abs() < 0.05, "mean {mean} is not centred on silence");
        }
    }

    #[test]
    fn the_same_seed_replays_the_same_noise() {
        for color in EVERY_COLOR {
            assert_eq!(filled(256, color, 42), filled(256, color, 42), "{color:?}");
        }
    }

    #[test]
    fn a_different_seed_or_channel_decorrelates() {
        assert_ne!(
            filled(256, NoiseColor::White, 42),
            filled(256, NoiseColor::White, 43)
        );
        assert_ne!(white(9, 0, 1), white(9, 1, 1));
    }

    /// The width the source is here for: the two sides are independent draws,
    /// not one draw copied, so the correlation between them is nothing like
    /// the 1.0 a duplicated channel would give — **in every colour**, because
    /// each side is coloured by its own filter.
    ///
    /// The bound is looser for a colour than for white and that is a fact
    /// about the estimator rather than about the width: a coloured draw's
    /// energy is bunched into its bottom octaves, so a finite window holds far
    /// fewer independent observations and the measured correlation wanders
    /// further from zero. A tenth is still an order of magnitude from a copied
    /// channel, and [`the_two_sides_are_two_draws_coloured_apart`] is where
    /// the mechanism itself is pinned down.
    #[test]
    fn the_two_sides_are_independent_draws() {
        for color in EVERY_COLOR {
            let buf = filled(1 << 16, color, 11);
            assert_ne!(buf.l, buf.r, "{color:?}");
            let n = buf.l.len() as f32;
            let dot: f32 = buf.l.iter().zip(&buf.r).map(|(l, r)| l * r).sum();
            let power = |c: &[f32]| c.iter().map(|s| s * s).sum::<f32>() / n;
            let correlation = (dot / n) / (power(&buf.l) * power(&buf.r)).sqrt();
            let bound = if color == NoiseColor::White {
                0.05
            } else {
                0.15
            };
            assert!(
                correlation.abs() < bound,
                "{color:?} correlated at {correlation}"
            );
        }
    }

    /// The mechanism behind that width, with no statistics in it: each side is
    /// its own draw through its own filter. Colouring one draw and copying it
    /// would be half the arithmetic and a mono source, and this is the test
    /// that would catch it.
    #[test]
    fn the_two_sides_are_two_draws_coloured_apart() {
        for color in EVERY_COLOR {
            let buf = filled(4096, color, 13);
            for (channel, side) in [(LEFT_CHANNEL, &buf.l), (RIGHT_CHANNEL, &buf.r)] {
                let mut alone = Coloring::new(color);
                let warmup = alone.warmup() as i64;
                for i in -warmup..0 {
                    alone.step(white(i, channel, 13));
                }
                let expected: Vec<f32> = (0..side.len())
                    .map(|i| alone.step(white(i as i64, channel, 13)))
                    .collect();
                assert_eq!(&expected, side, "{color:?} on channel {channel}");
            }
        }
    }

    /// The promise `color` states: changing colour changes the character and
    /// not the fader. Half a decibel is well inside what a listener would call
    /// the same level, and a scale constant that had drifted would miss it.
    #[test]
    fn every_colour_arrives_at_white_s_level() {
        let white = rms(&filled(1 << 16, NoiseColor::White, 3).l);
        for color in [NoiseColor::Pink, NoiseColor::Brown] {
            let level = rms(&filled(1 << 16, color, 3).l);
            let db = gain_to_db_f32(level / white);
            assert!(db.abs() < 0.5, "{color:?} is {db:+.2} dB from white");
        }
    }

    /// Brown is a *stationary* signal and not a random walk: its excursion
    /// stops growing with the length of the note. An unleaky integrator peaks
    /// four times higher over a million samples than over sixteen thousand,
    /// and this is where that shows up.
    #[test]
    fn brown_does_not_drift_further_the_longer_it_runs() {
        let peak = |n: usize| {
            filled(n, NoiseColor::Brown, 5)
                .l
                .iter()
                .fold(0.0f32, |m, s| m.max(s.abs()))
        };
        let (short, long) = (peak(1 << 14), peak(1 << 18));
        assert!(long < 1.6 * short, "grew from {short} to {long}");
        // And the walk stays centred: no slow ramp riding under the noise.
        let buf = filled(1 << 16, NoiseColor::Brown, 5);
        let window = 4096;
        for (at, chunk) in buf.l.chunks_exact(window).enumerate() {
            let mean = chunk.iter().sum::<f32>() / window as f32;
            assert!(mean.abs() < 1.0, "window {at} sits at {mean}");
        }
    }

    /// The warm-up, asserted where it cannot hide: sample zero is **not** what
    /// a filter starting from rest would have produced from that same draw. An
    /// unwarmed source begins at silence and climbs out of it over the ten
    /// milliseconds a listener identifies an impact by, and the climb is
    /// mostly in the slowest pole — which is to say it is a change of
    /// *spectrum* far more than of level, and a level check alone lets it
    /// through.
    #[test]
    fn a_coloured_source_does_not_begin_at_rest() {
        for color in [NoiseColor::Pink, NoiseColor::Brown] {
            let mut cold = Coloring::new(color);
            let from_rest = cold.step(white(0, LEFT_CHANNEL, 17));
            assert_ne!(
                filled(64, color, 17).l[0],
                from_rest,
                "{color:?} started from rest"
            );
        }
        // White has nothing to settle, so it *is* the cold value — the arm
        // that keeps an existing recipe's samples where they were.
        let mut cold = Coloring::new(NoiseColor::White);
        assert_eq!(
            filled(64, NoiseColor::White, 17).l[0],
            cold.step(white(0, LEFT_CHANNEL, 17))
        );
    }

    /// And the audible half of the same thing: a coloured source is already at
    /// its own level in the first hundredth of a second.
    #[test]
    fn a_coloured_source_starts_at_the_level_it_continues_at() {
        for color in [NoiseColor::Pink, NoiseColor::Brown] {
            let buf = filled(1 << 15, color, 17);
            let opening = rms(&buf.l[..441]);
            let settled = rms(&buf.l[4410..]);
            let db = gain_to_db_f32(opening / settled);
            assert!(
                db.abs() < 6.0,
                "{color:?} opens {db:+.2} dB off its own level"
            );
        }
    }

    /// The default is the white this source has always been, sample for
    /// sample — nothing about colour arriving may change what an existing
    /// recipe renders to.
    #[test]
    fn white_is_the_bare_draw_it_always_was() {
        let buf = filled(512, NoiseColor::White, 9);
        for (i, sample) in buf.l.iter().enumerate() {
            assert_eq!(*sample, white(i as i64, LEFT_CHANNEL, 9));
        }
        for (i, sample) in buf.r.iter().enumerate() {
            assert_eq!(*sample, white(i as i64, RIGHT_CHANNEL, 9));
        }
        assert_eq!(NoiseColor::default(), NoiseColor::White);
    }

    /// Each colour is a different signal, and none of them is another wearing
    /// a scale factor.
    #[test]
    fn the_three_colours_are_three_signals() {
        let one = |color| filled(4096, color, 21).l;
        assert_ne!(one(NoiseColor::White), one(NoiseColor::Pink));
        assert_ne!(one(NoiseColor::Pink), one(NoiseColor::Brown));
        assert_ne!(one(NoiseColor::White), one(NoiseColor::Brown));
    }
}
