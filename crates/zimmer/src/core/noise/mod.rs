pub(crate) mod color;

use crate::hash::unit2;
use crate::patch::NoiseColor;
use crate::stereo::Stereo;
use color::Coloring;

const LEFT_CHANNEL: u64 = 0;
/// ASCII "N2": a channel of its own, so the right side is uncorrelated with the left.
const RIGHT_CHANNEL: u64 = 0x4e32;

/// `i` is signed so a colour filter can be primed on draws before the note ([`color::WARMUP`]).
#[inline]
pub(crate) fn white(i: i64, channel: u64, seed: u64) -> f32 {
    unit2(i, 0, channel, seed) * 2.0 - 1.0
}

/// Each side is its own draw through its own filter: one draw coloured and copied would be mono.
pub(crate) fn fill(out: &mut Stereo, color: NoiseColor, seed: u64) {
    color_into(&mut out.l, LEFT_CHANNEL, color, seed);
    color_into(&mut out.r, RIGHT_CHANNEL, color, seed);
}

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

    #[test]
    fn the_two_sides_are_independent_draws() {
        for color in EVERY_COLOR {
            let buf = filled(1 << 16, color, 11);
            assert_ne!(buf.l, buf.r, "{color:?}");
            let n = buf.l.len() as f32;
            let dot: f32 = buf.l.iter().zip(&buf.r).map(|(l, r)| l * r).sum();
            let power = |c: &[f32]| c.iter().map(|s| s * s).sum::<f32>() / n;
            let correlation = (dot / n) / (power(&buf.l) * power(&buf.r)).sqrt();
            // Looser for a colour: its energy bunches low, so the window holds fewer independent
            // observations.
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

    #[test]
    fn every_colour_arrives_at_white_s_level() {
        let white = rms(&filled(1 << 16, NoiseColor::White, 3).l);
        for color in [NoiseColor::Pink, NoiseColor::Brown] {
            let level = rms(&filled(1 << 16, color, 3).l);
            let db = gain_to_db_f32(level / white);
            assert!(db.abs() < 0.5, "{color:?} is {db:+.2} dB from white");
        }
    }

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
        let buf = filled(1 << 16, NoiseColor::Brown, 5);
        let window = 4096;
        for (at, chunk) in buf.l.chunks_exact(window).enumerate() {
            let mean = chunk.iter().sum::<f32>() / window as f32;
            assert!(mean.abs() < 1.0, "window {at} sits at {mean}");
        }
    }

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
        // White has nothing to settle, so it *is* the cold value — the arm that keeps an existing
        // recipe's samples where they were.
        let mut cold = Coloring::new(NoiseColor::White);
        assert_eq!(
            filled(64, NoiseColor::White, 17).l[0],
            cold.step(white(0, LEFT_CHANNEL, 17))
        );
    }

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

    #[test]
    fn the_three_colours_are_three_signals() {
        let one = |color| filled(4096, color, 21).l;
        assert_ne!(one(NoiseColor::White), one(NoiseColor::Pink));
        assert_ne!(one(NoiseColor::Pink), one(NoiseColor::Brown));
        assert_ne!(one(NoiseColor::White), one(NoiseColor::Brown));
    }
}
