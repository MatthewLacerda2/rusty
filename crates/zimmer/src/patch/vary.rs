//! vary: another take of the same one-shot, from one number (#1038).
//!
//! A one-shot's `seed` re-rolls its noise grain and where its oscillators
//! start, and nothing else: two seeds of one pop are two files and one sound
//! to the ear (the 5 ms envelopes of #1001's kit correlated at 0.96–1.00).
//! In a song a performance's `humanize` varies each hit; a single hit has no
//! performance around it. So a one-shot that asks for it is played from a
//! **scattered copy** of its patch instead: the numbers that set a struck
//! sound's character — how fast it speaks and dies, where its filter sits,
//! how far its pitch bounces — each nudged by a seeded draw, as far as the
//! amount allows.
//!
//! A draw is a multiplication by `2^(amount × reach × u)`, `u` in `-1..1`,
//! so a time of zero stays an instant and a falling bounce never turns into
//! a rising one. Each number has a **fixed slot** in the draw, so a patch
//! that gains a filter keeps the amp envelope every seed gave it before, and
//! a [layer](super::Layer) is scattered the same way on slots of its own.
//!
//! Free and deterministic like everything here: the same patch, amount and
//! seed give the same copy on every machine, and an amount of zero gives the
//! patch back untouched, which is what every recipe written before the field
//! still means.

use super::{Adsr, Filter, Patch, PitchEnv};
use crate::hash::unit2;

/// Hash channel the scatter draws on, apart from the noise and the start
/// phases that read the same seed.
const VARY_CHANNEL: u64 = 0x5641; // "VA"

/// How far a timing moves at full `vary`, in octaves: half as long to twice
/// as long — a snap to a thud's bloom, and no further.
const TIME_REACH: f32 = 1.0;
/// How far a filter's cutoff moves at full `vary`, in octaves.
const CUTOFF_REACH: f32 = 1.0;
/// How far a pitch envelope's depth moves at full `vary`, in octaves of its
/// size: `log2(1.5)`, so a twelve-semitone bounce lands between eight and
/// eighteen.
const BOUNCE_REACH: f32 = 0.585;

/// The first slot of each stage's draws: the amp envelope's three open the
/// path (slots 0–2), then one cutoff and three timings for the filter, one
/// depth and three timings for the pitch envelope.
///
/// Each is counted from the first slot of the path it scatters: the patch's
/// own path starts at zero, and layer `i` at `(i + 1) × PATH_SLOTS`, after
/// every slot the patch's own path takes, so a patch that gains a layer keeps
/// every draw a seed gave it before.
const CUTOFF_SLOT: i64 = 3;
const FILTER_SLOTS: i64 = 4;
const BOUNCE_SLOT: i64 = 7;
const PITCH_SLOTS: i64 = 8;
/// How many slots one path takes.
const PATH_SLOTS: i64 = 11;

impl Patch {
    /// This patch as another take of itself: its envelope timings, its
    /// filter's cutoff and its pitch envelope's depth each scattered by a
    /// draw from `seed`, as far as `amount` (`0..=1`) allows.
    ///
    /// At full amount a timing lands anywhere from half to twice what was
    /// written and a cutoff an octave either side; `0`, or anything that is
    /// not a number, returns the patch unchanged. Past one is one. It is what
    /// a one-shot's `vary` field asks for, and the caller hands it the seed
    /// the note is struck with, so one number is one take.
    pub fn varied(&self, amount: f32, seed: u64) -> Patch {
        let mut patch = self.clone();
        let amount = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if amount == 0.0 {
            return patch;
        }
        let draw = Draw { amount, seed };
        draw.path(&mut patch.amp, &mut patch.filter, &mut patch.pitch_env, 0);
        for (index, layer) in (1..).zip(&mut patch.layers) {
            let first = index * PATH_SLOTS;
            draw.path(
                &mut layer.amp,
                &mut layer.filter,
                &mut layer.pitch_env,
                first,
            );
        }
        patch
    }
}

/// One take's scatter: how much, and from which seed.
struct Draw {
    amount: f32,
    seed: u64,
}

impl Draw {
    /// `value` moved by up to `reach` octaves either way, by the draw in
    /// `slot`.
    fn scale(&self, value: f32, slot: i64, reach: f32) -> f32 {
        let u = 2.0 * unit2(slot, 0, VARY_CHANNEL, self.seed) - 1.0;
        value * (self.amount * reach * u).exp2()
    }

    /// One path's envelopes, cutoff and bounce, on the slots from `first`.
    fn path(
        &self,
        amp: &mut Adsr,
        filter: &mut Option<Filter>,
        pitch_env: &mut Option<PitchEnv>,
        first: i64,
    ) {
        // The amp envelope's three draws open the path, on its first slots.
        self.times(amp, first);
        if let Some(filter) = filter {
            filter.cutoff = self.scale(filter.cutoff, first + CUTOFF_SLOT, CUTOFF_REACH);
            self.times(&mut filter.adsr, first + FILTER_SLOTS);
        }
        if let Some(env) = pitch_env {
            env.semitones = self.scale(env.semitones, first + BOUNCE_SLOT, BOUNCE_REACH);
            self.times(&mut env.adsr, first + PITCH_SLOTS);
        }
    }

    /// An envelope's three timings, each on its own draw. The sustain is a
    /// level rather than a time, and a struck sound's is usually zero, so it
    /// is left where it was written.
    fn times(&self, adsr: &mut Adsr, first: i64) {
        adsr.a = self.scale(adsr.a, first, TIME_REACH);
        adsr.d = self.scale(adsr.d, first + 1, TIME_REACH);
        adsr.r = self.scale(adsr.r, first + 2, TIME_REACH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Filter, FilterKind, Osc, PitchEnv, Slope, Source, Wave};

    /// A pop: a sine falling an octave onto its note, through a filter.
    fn pop() -> Patch {
        let adsr = Adsr {
            a: 0.001,
            d: 0.07,
            s: 0.0,
            r: 0.02,
            curve: 4.0,
        };
        Patch {
            layers: Vec::new(),
            source: Source::OscStack {
                oscs: vec![Osc {
                    wave: Wave::Sine,
                    detune_cents: 0.0,
                    gain: 1.0,
                    octave: 0,
                    voices: 1,
                    spread: 12.0,
                    width: 0.0,
                    pulse_width: 0.5,
                }],
            },
            amp: adsr,
            filter: Some(Filter {
                kind: FilterKind::Lowpass,
                cutoff: 2000.0,
                slope: Slope::default(),
                resonance: 0.0,
                env_octaves: 0.0,
                vel_octaves: 0.0,
                key_track: 0.0,
                adsr,
            }),
            pitch_env: Some(PitchEnv {
                semitones: -12.0,
                adsr,
            }),
            lfo: None,
            lfo2: None,
            fx: Vec::new(),
        }
    }

    /// The promise every existing recipe rests on: no amount, no change.
    #[test]
    fn nothing_to_vary_is_the_patch_as_written() {
        for amount in [0.0, -0.5, f32::NAN, f32::NEG_INFINITY] {
            assert_eq!(pop().varied(amount, 9), pop(), "amount {amount}");
        }
    }

    /// Every number lands inside its reach, on the side of zero it started.
    #[test]
    fn a_full_take_stays_inside_its_reach() {
        for seed in 0..64 {
            let take = pop().varied(1.0, seed);
            let filter = take.filter.expect("still filtered");
            let bounce = take.pitch_env.expect("still bouncing").semitones;
            for (time, written) in [(take.amp.d, 0.07), (filter.adsr.a, 0.001)] {
                assert!((written / 2.0..=written * 2.0).contains(&time), "{time}");
            }
            assert!(
                (1000.0..=4000.0).contains(&filter.cutoff),
                "{}",
                filter.cutoff
            );
            assert!((-18.0..=-8.0).contains(&bounce), "bounce {bounce}");
            assert_eq!(take.amp.s, 0.0, "a level is not a time");
        }
        // And past one is one.
        assert_eq!(pop().varied(4.0, 3), pop().varied(1.0, 3));
    }

    /// Two seeds are two takes, and a smaller amount is a smaller nudge.
    #[test]
    fn each_seed_is_a_different_take_and_amount_is_how_far() {
        let decays: Vec<f32> = (0..8).map(|seed| pop().varied(1.0, seed).amp.d).collect();
        let (low, high) = decays
            .iter()
            .fold((f32::MAX, 0.0f32), |(l, h), d| (l.min(*d), h.max(*d)));
        assert!(high / low > 1.3, "eight takes barely moved: {decays:?}");
        let gentle = pop().varied(0.1, 5).amp.d / 0.07;
        assert!(
            (0.93..=1.072).contains(&gentle),
            "a tenth moved by {gentle}"
        );
        assert_eq!(pop().varied(0.6, 5), pop().varied(0.6, 5), "and replays");
    }

    /// A stage's draws are its own: adding a filter does not re-roll the amp
    /// envelope a seed already gave.
    #[test]
    fn a_stage_keeps_its_draws_whatever_else_the_patch_has() {
        let bare = Patch {
            filter: None,
            pitch_env: None,
            ..pop()
        };
        assert_eq!(bare.varied(0.7, 21).amp, pop().varied(0.7, 21).amp);
    }

    /// A layer is scattered too, on draws of its own, and gaining one moves
    /// nothing a seed already gave the patch's own path.
    #[test]
    fn a_layer_takes_its_own_draws_after_the_patch_s() {
        let layer = crate::patch::Layer {
            source: pop().source,
            amp: pop().amp,
            filter: pop().filter,
            pitch_env: pop().pitch_env,
            gain: 1.0,
        };
        let layered = Patch {
            layers: vec![layer],
            ..pop()
        };
        let take = layered.varied(1.0, 4);
        let Patch { layers, .. } = take.clone();
        assert_eq!(
            Patch {
                layers: Vec::new(),
                ..take
            },
            pop().varied(1.0, 4)
        );
        assert_ne!(
            layers[0].amp,
            pop().varied(1.0, 4).amp,
            "the layer drew its own"
        );
        assert_ne!(layers[0].amp, pop().amp, "and was scattered");
    }

    /// One take, pinned to the number: the patch's own path draws what it drew
    /// before layers existed, and each layer's slots sit after it. A slot that
    /// moves re-rolls every existing seed's take, and only exact numbers see it.
    #[test]
    fn one_take_lands_on_the_numbers_its_slots_give() {
        let layer = crate::patch::Layer {
            source: pop().source,
            amp: pop().amp,
            filter: pop().filter,
            pitch_env: pop().pitch_env,
            gain: 1.0,
        };
        let take = Patch {
            layers: vec![layer],
            ..pop()
        }
        .varied(1.0, 4);
        let numbers = |amp: Adsr, filter: Option<crate::patch::Filter>, env: Option<PitchEnv>| {
            let (filter, env) = (filter.expect("filtered"), env.expect("bouncing"));
            [
                amp.a,
                amp.d,
                amp.r,
                filter.cutoff,
                filter.adsr.a,
                filter.adsr.d,
                filter.adsr.r,
                env.semitones,
                env.adsr.a,
                env.adsr.d,
                env.adsr.r,
            ]
        };
        let own = [
            0.000_528_609_8,
            0.133_729_93,
            0.019_052_863,
            1580.8652,
            0.001_739_532_3,
            0.107_752_03,
            0.035_319_33,
            -8.031_763,
            0.000_803_819_5,
            0.105_488_114,
            0.031_174_842,
        ];
        let layered = [
            0.001_143_434_9,
            0.135_319_35,
            0.028_778_698,
            1674.0792,
            0.001_649_295_7,
            0.125_252_7,
            0.017_573_703,
            -12.432_24,
            0.000_506_302_4,
            0.041_075_647,
            0.011_167_941,
        ];
        let l = &take.layers[0];
        for (got, want) in numbers(take.amp, take.filter, take.pitch_env)
            .into_iter()
            .zip(own)
            .chain(
                numbers(l.amp, l.filter, l.pitch_env)
                    .into_iter()
                    .zip(layered),
            )
        {
            assert!((got / want - 1.0).abs() < 1e-5, "{got} against {want}");
        }
    }
}
