//! Each draw scales a number by `2^(amount × reach × u)`, `u` in `-1..1`: a zero stays zero
//! and a pitch bounce keeps its sign.

use super::{Adsr, Filter, Patch, PitchEnv};
use crate::hash::unit2;

/// Hash channel "VA", apart from the ones noise and start phases draw on.
const VARY_CHANNEL: u64 = 0x5641;

// Octaves either way at full `vary`: a timing lands at half to twice, a cutoff an octave off.
const TIME_REACH: f32 = 1.0;
const CUTOFF_REACH: f32 = 1.0;
/// `log2(1.5)` octaves of a pitch envelope's depth: a 12-semitone bounce lands in 8..18.
const BOUNCE_REACH: f32 = 0.585;

// Fixed draw slots: moving one re-rolls every seed's existing take.
const CUTOFF_SLOT: i64 = 3;
const FILTER_SLOTS: i64 = 4;
const BOUNCE_SLOT: i64 = 7;
const PITCH_SLOTS: i64 = 8;
const PATH_SLOTS: i64 = 11;

impl Patch {
    /// Only rusty-zimmer's own tests call it: no `Sound.*` key reaches it.
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

struct Draw {
    amount: f32,
    seed: u64,
}

impl Draw {
    fn scale(&self, value: f32, slot: i64, reach: f32) -> f32 {
        let u = 2.0 * unit2(slot, 0, VARY_CHANNEL, self.seed) - 1.0;
        value * (self.amount * reach * u).exp2()
    }

    fn path(
        &self,
        amp: &mut Adsr,
        filter: &mut Option<Filter>,
        pitch_env: &mut Option<PitchEnv>,
        first: i64,
    ) {
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

    #[test]
    fn nothing_to_vary_is_the_patch_as_written() {
        for amount in [0.0, -0.5, f32::NAN, f32::NEG_INFINITY] {
            assert_eq!(pop().varied(amount, 9), pop(), "amount {amount}");
        }
    }

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
        assert_eq!(pop().varied(4.0, 3), pop().varied(1.0, 3));
    }

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

    #[test]
    fn a_stage_keeps_its_draws_whatever_else_the_patch_has() {
        let bare = Patch {
            filter: None,
            pitch_env: None,
            ..pop()
        };
        assert_eq!(bare.varied(0.7, 21).amp, pop().varied(0.7, 21).amp);
    }

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

    const OWN: [f32; 11] = [
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

    const LAYERED: [f32; 11] = [
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

    fn numbers(
        amp: Adsr,
        filter: Option<crate::patch::Filter>,
        env: Option<PitchEnv>,
    ) -> [f32; 11] {
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
    }

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
        let l = &take.layers[0];
        for (got, want) in numbers(take.amp, take.filter, take.pitch_env)
            .into_iter()
            .zip(OWN)
            .chain(
                numbers(l.amp, l.filter, l.pitch_env)
                    .into_iter()
                    .zip(LAYERED),
            )
        {
            assert!((got / want - 1.0).abs() < 1e-5, "{got} against {want}");
        }
    }
}
