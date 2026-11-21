//! The per-sample curves a note follows: pitch, cutoff, and the tremolo gain.

use super::env;
use super::lfo::Lfos;
use super::RATE;
use crate::note::Glide;
use crate::patch::{Filter, LfoTarget, PitchEnv};

const MAX_PITCH_OFFSET: f32 = 120.0;

pub(super) fn pitch_track(
    lfos: &Lfos,
    pitch_env: Option<PitchEnv>,
    glide: Option<Glide>,
    base: f32,
    gate: f32,
    n: usize,
) -> Vec<f32> {
    let vibrato = lfos.aims_at(LfoTarget::Pitch);
    let sweep = pitch_env.map(|p| (p.semitones, env::track(&p.adsr, gate, n, RATE)));
    let slide = glide
        .filter(|g| g.semitones != 0.0 && g.seconds > 0.0)
        .map(|g| (g.semitones, g.seconds * RATE));
    if !vibrato && sweep.is_none() && slide.is_none() {
        return vec![base; n];
    }
    (0..n)
        .map(|i| {
            let mut semitones = 0.0;
            if vibrato {
                semitones += lfos.offset(LfoTarget::Pitch, i);
            }
            if let Some((amount, envelope)) = &sweep {
                semitones += amount * envelope[i];
            }
            if let Some((from, samples)) = slide {
                let left = 1.0 - i as f32 / samples;
                if left > 0.0 {
                    semitones += from * left;
                }
            }
            base * (semitones.clamp(-MAX_PITCH_OFFSET, MAX_PITCH_OFFSET) / 12.0).exp2()
        })
        .collect()
}

pub(super) fn cutoff_track(
    f: &Filter,
    lfos: &Lfos,
    gate: f32,
    n: usize,
    velocity: f32,
    midi: f32,
    base: Option<&[f32]>,
) -> Vec<f32> {
    let envelope = env::track(&f.adsr, gate, n, RATE);
    let followed = f.key_track.clamp(0.0, 1.0) * (midi - 60.0) / 12.0;
    let struck = f.vel_octaves * velocity + followed;
    (0..n)
        .map(|i| {
            let octaves = struck + f.env_octaves * envelope[i];
            let wobble = lfos.offset(LfoTarget::Cutoff, i);
            let at = base.and_then(|base| base.get(i)).copied();
            at.unwrap_or(f.cutoff) * (octaves + wobble).exp2()
        })
        .collect()
}

#[cfg(test)]
mod key;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Adsr, Lfo};

    const BASE: f32 = 440.0;

    const RATE_HZ: f32 = 5.0;

    const PEAK: usize = 2_205;

    const TROUGH: usize = 6_615;

    const CYCLE: usize = 8_820;

    fn lfo(depth: f32, target: LfoTarget) -> Lfo {
        Lfo::new(RATE_HZ, depth, target)
    }

    fn lfos(lfo: Option<Lfo>, n: usize) -> Lfos {
        Lfos::of(lfo.as_ref(), 0, None, n)
    }

    fn vibrato_track(lfo: Option<Lfo>, n: usize) -> Vec<f32> {
        pitch_track(&lfos(lfo, n), None, None, BASE, 1.0, n)
    }

    fn sweep(semitones: f32, adsr: Adsr) -> PitchEnv {
        PitchEnv { semitones, adsr }
    }

    fn falling() -> Adsr {
        Adsr {
            a: 0.0,
            d: 1.0,
            s: 0.0,
            r: 0.0,
            curve: 0.0,
        }
    }

    fn held() -> Adsr {
        Adsr {
            a: 0.0,
            d: 0.0,
            s: 1.0,
            r: 0.0,
            curve: 0.0,
        }
    }

    fn sweep_track(semitones: f32, adsr: Adsr, n: usize) -> Vec<f32> {
        pitch_track(
            &lfos(None, n),
            Some(sweep(semitones, adsr)),
            None,
            BASE,
            1.0,
            n,
        )
    }

    #[test]
    fn a_pitch_envelope_starts_off_the_note_and_lands_on_it() {
        let track = sweep_track(24.0, falling(), 44_100);
        assert_eq!(track[0], BASE, "the envelope is still at zero");
        assert!(track[1] > 1_759.0, "two octaves up, at {}", track[1]);
        assert_eq!(track[22_050], 880.0, "one octave left at half the decay");
        assert!(
            (track[44_099] - BASE).abs() < 0.1,
            "and settles on the note, at {}",
            track[44_099]
        );
    }

    #[test]
    fn a_negative_amount_sweeps_up_onto_the_note_from_below() {
        let track = sweep_track(-12.0, held(), 8);
        assert_eq!(track[1], 220.0, "an octave below at full level");
    }

    #[test]
    fn a_sweep_and_a_vibrato_add_in_semitones() {
        let track = pitch_track(
            &lfos(Some(lfo(12.0, LfoTarget::Pitch)), CYCLE),
            Some(sweep(12.0, held())),
            None,
            BASE,
            1.0,
            CYCLE,
        );
        assert_eq!(track[PEAK], 1_760.0, "an octave of sweep plus one of bend");
        assert_eq!(track[TROUGH], BASE, "and the bend cancels the sweep");
    }

    #[test]
    fn a_sweep_needs_no_lfo_to_reach_the_track() {
        assert_eq!(sweep_track(12.0, held(), 8)[1], 880.0);
    }

    #[test]
    fn an_absurd_sweep_is_clamped_instead_of_reaching_infinity() {
        for semitones in [1e9, -1e9, f32::MAX, f32::MIN] {
            let track = sweep_track(semitones, held(), 16);
            for (i, f) in track.iter().enumerate() {
                assert!(f.is_finite() && *f > 0.0, "{semitones} at {i} gave {f}");
            }
        }
    }

    fn glide_track(semitones: f32, seconds: f32, n: usize) -> Vec<f32> {
        pitch_track(
            &lfos(None, n),
            None,
            Some(Glide { semitones, seconds }),
            BASE,
            1.0,
            n,
        )
    }

    #[test]
    fn a_glide_starts_off_the_note_in_the_direction_its_sign_says() {
        let above = glide_track(12.0, 0.1, 4_411);
        assert_eq!(above[0], 880.0, "an octave above, at the first sample");
        assert!((BASE..880.0).contains(&above[2_205]), "not on the way down");
        assert_eq!(above[4_410], BASE, "and not arrived");

        let below = glide_track(-12.0, 0.1, 4_411);
        assert_eq!(below[0], 220.0, "an octave below, at the first sample");
        assert!((220.0..BASE).contains(&below[2_205]), "not on the way up");
        assert_eq!(below[4_410], BASE, "and not arrived");
    }

    #[test]
    fn a_glide_stops_bending_the_note_once_it_has_arrived() {
        let track = glide_track(7.0, 0.01, 4_410);
        assert!(track[440] > BASE, "already given up with a sample to go");
        for (i, hz) in track.iter().enumerate().skip(441) {
            assert_eq!(*hz, BASE, "still bent at {i}");
        }
    }

    #[test]
    fn a_glide_of_nothing_bends_nothing() {
        for glide in [(0.0, 0.1), (12.0, 0.0), (12.0, -1.0)] {
            let track = glide_track(glide.0, glide.1, 8);
            assert_eq!(track, vec![BASE; 8], "{glide:?} bent the note");
        }
    }

    #[test]
    fn a_glide_adds_to_a_sweep_rather_than_replacing_it() {
        let track = pitch_track(
            &lfos(None, 0),
            Some(sweep(12.0, held())),
            Some(Glide {
                semitones: 12.0,
                seconds: 0.1,
            }),
            BASE,
            1.0,
            4_411,
        );
        assert_eq!(track[0], 880.0, "the slide alone, the envelope at zero");
        assert!((track[2_205] - 1_244.5).abs() < 0.1, "at {}", track[2_205]);
        assert_eq!(track[4_410], 880.0, "the sweep alone, the slide arrived");
    }

    #[test]
    fn the_first_sample_is_the_played_pitch_whatever_the_depth() {
        for depth in [0.0, 2.0, 12.0, -7.0, 24.0] {
            let track = vibrato_track(Some(lfo(depth, LfoTarget::Pitch)), 8);
            assert_eq!(track[0], BASE, "depth {depth}");
        }
    }

    #[test]
    fn twelve_semitones_bends_a_whole_octave_each_way() {
        let track = vibrato_track(Some(lfo(12.0, LfoTarget::Pitch)), CYCLE);
        assert_eq!(track[PEAK], 880.0);
        assert_eq!(track[TROUGH], 220.0);
    }

    #[test]
    fn twice_the_depth_is_twice_the_octaves() {
        let track = vibrato_track(Some(lfo(24.0, LfoTarget::Pitch)), CYCLE);
        assert_eq!(track[PEAK], 1760.0);
        assert_eq!(track[TROUGH], 110.0);
    }

    #[test]
    fn an_lfo_aimed_elsewhere_leaves_the_pitch_flat() {
        for target in [LfoTarget::Cutoff, LfoTarget::Amp] {
            flat(&vibrato_track(Some(lfo(12.0, target)), CYCLE), CYCLE);
        }
    }

    #[test]
    fn no_lfo_is_a_flat_track_of_the_length_asked_for() {
        flat(&vibrato_track(None, 512), 512);
    }

    fn flat(track: &[f32], n: usize) {
        assert_eq!(track.len(), n);
        for (i, f) in track.iter().enumerate() {
            assert_eq!(*f, BASE, "sample {i}");
        }
    }
}
