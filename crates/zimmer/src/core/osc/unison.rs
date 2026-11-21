//! unison: one written oscillator, several sounding ones.
//!
//! Detuning copies of a wave against each other is the oldest trick in
//! subtractive synthesis and the whole of the supersaw. What this module owns
//! is the four numbers that turn `voices` into that: where each copy sits in
//! pitch, what each one weighs, where in its cycle each one begins, and where
//! in the stereo field it sounds.
//!
//! The first three have to be per **voice** or the feature is not unison. Copies at
//! one pitch are one oscillator; copies at one gain are that oscillator turned
//! up; and copies at one phase are that oscillator turned up for the first few
//! tens of milliseconds, which is exactly the stretch the ear identifies a
//! sound by.

use crate::hash::unit2;
use crate::patch::Osc;

/// Hash channel the start phases draw on, so a stack never mirrors the noise a
/// `noise` source or a Karplus excitation draws from the same note seed.
const PHASE_CHANNEL: u64 = 0x4f53; // "OS"

/// How many copies of `osc` actually sound.
///
/// `Patch::validate` refuses a count outside `1..=MAX_VOICES`, so the floor
/// here can only be reached by calling the renderer directly — and an
/// oscillator that rendered no voices at all would be a silent stack that
/// passed every gain check.
pub(super) fn voices(osc: &Osc) -> usize {
    osc.voices.max(1)
}

/// One unison voice's weight in the stack: the oscillator's own share of the
/// mix, split between its copies.
///
/// The division is what makes `voices` a thickness control rather than a
/// fader. Without it a seven-voice entry would be seven times the amplitude
/// the same entry had at one voice, and every gain written anywhere near it
/// would have to move to pay for a word.
pub(super) fn voice_gain(osc: &Osc, norm: f32) -> f32 {
    osc.gain.max(0.0) * norm / voices(osc) as f32
}

/// Where voice `voice` sits relative to the oscillator's own detune, in cents.
///
/// Evenly spaced across the full `spread` and **centred on zero**, so the
/// spread widens the sound without moving the pitch it was written at: two
/// voices sit at ±half the spread, three put one voice dead centre, and a
/// single voice is exactly where it always was however wide the spread says.
pub(super) fn voice_detune(osc: &Osc, voice: usize) -> f32 {
    let voices = voices(osc);
    if voices < 2 {
        return 0.0;
    }
    osc.spread * (voice as f32 / (voices - 1) as f32 - 0.5)
}

/// Where voice `voice` sits on the pan axis, in `-1..=1`.
///
/// Spaced exactly as [`voice_detune`] spaces the pitches — evenly across
/// `-width..=width`, lowest-pitched voice on the left — so pitch and place
/// fan out together, which is how a hardware unison spread is wired. A lone
/// voice, or a `width` of zero, is dead centre: the `0.0` that
/// [`pan_gains`](crate::stereo::pan_gains) turns into an exact unity on both
/// sides, which is what keeps an unspread oscillator the samples it always
/// was. A width that is not a number is no width at all.
pub(super) fn voice_pan(osc: &Osc, voice: usize) -> f32 {
    let voices = voices(osc);
    let width = if osc.width.is_finite() {
        osc.width.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if voices < 2 || width == 0.0 {
        return 0.0;
    }
    width * (2.0 * voice as f32 / (voices - 1) as f32 - 1.0)
}

/// Where voice `voice` of oscillator `index`, in a note seeded `seed`, starts
/// in its cycle, in `0..1`.
///
/// Per **voice** and not merely per oscillator, which is the half of unison
/// that is not the detune: copies that all start locked together are one
/// oscillator at N times the gain until the detune has pulled them apart.
/// Voice zero draws exactly what a lone oscillator always drew.
pub(super) fn start_phase(index: usize, voice: usize, seed: u64) -> f32 {
    unit2(index as i64, voice as i64, PHASE_CHANNEL, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{MAX_VOICES, Wave};

    /// An oscillator widened into `voices` copies `spread` cents apart.
    fn unison(voices: usize, spread: f32) -> Osc {
        Osc {
            wave: Wave::Saw,
            detune_cents: 0.0,
            gain: 1.0,
            octave: 0,
            voices,
            spread,
            width: 0.0,
            pulse_width: 0.5,
        }
    }

    #[test]
    fn a_detuned_pair_does_not_start_in_step() {
        assert_ne!(start_phase(0, 0, 9), start_phase(1, 0, 9));
        assert_ne!(start_phase(0, 0, 9), start_phase(0, 0, 10));
        for index in 0..4 {
            let phase = start_phase(index, 0, 9);
            assert!((0.0..1.0).contains(&phase), "phase {phase} is not a phase");
        }
    }

    /// The half of unison that is not the detune: seven voices are seven
    /// different places in the cycle, not one place seven times. A draw that
    /// ignored the voice would pass every pitch and level test there is.
    #[test]
    fn every_unison_voice_starts_somewhere_else() {
        let phases: Vec<f32> = (0..MAX_VOICES).map(|v| start_phase(1, v, 9)).collect();
        for phase in &phases {
            assert!((0.0..1.0).contains(phase), "phase {phase} is not a phase");
        }
        for (at, phase) in phases.iter().enumerate() {
            for other in &phases[at + 1..] {
                assert_ne!(phase, other, "two voices start together in {phases:?}");
            }
        }
        // And the first voice is where a lone oscillator has always started,
        // which is what keeps a one-voice entry the samples it always was.
        assert_eq!(phases[0], unit2(1, 0, PHASE_CHANNEL, 9));
    }

    /// Seven voices are seven distinct pitches, evenly spaced across the
    /// spread and centred on the oscillator's own detune — so widening a sound
    /// never transposes it.
    #[test]
    fn unison_voices_are_spread_evenly_around_the_written_pitch() {
        let cents: Vec<f32> = (0..MAX_VOICES)
            .map(|v| voice_detune(&unison(MAX_VOICES, 21.0), v))
            .collect();
        assert!((cents[0] + 10.5).abs() < 1e-4, "lowest at {}", cents[0]);
        assert!((cents[6] - 10.5).abs() < 1e-4, "highest at {}", cents[6]);
        assert!(
            cents[3].abs() < 1e-4,
            "the middle voice moved to {}",
            cents[3]
        );
        for pair in cents.windows(2) {
            assert!((pair[1] - pair[0] - 3.5).abs() < 1e-4, "uneven: {cents:?}");
        }
        assert!(
            cents.iter().sum::<f32>().abs() < 1e-3,
            "the spread is off centre: {cents:?}"
        );
        // A lone voice sits exactly where it was written, however wide the
        // spread says — there is nothing to spread it against.
        assert_eq!(voice_detune(&unison(1, 50.0), 0), 0.0);
        // Two voices take the ends and nothing in between.
        let pair = unison(2, 20.0);
        assert_eq!(
            [voice_detune(&pair, 0), voice_detune(&pair, 1)],
            [-10.0, 10.0]
        );
    }

    /// The normalisation, as arithmetic rather than as a peak: an oscillator's
    /// weight is split between its copies, so seven voices are a seventh each
    /// and the entry keeps the share of the stack it had.
    #[test]
    fn a_widened_oscillator_keeps_its_share_of_the_stack() {
        assert_eq!(voice_gain(&unison(1, 12.0), 1.0), 1.0);
        for count in 2..=MAX_VOICES {
            let each = voice_gain(&unison(count, 12.0), 1.0);
            assert!(
                (each * count as f32 - 1.0).abs() < 1e-6,
                "{count} voices at {each} each do not add up to one"
            );
        }
        // A stack normalises before unison splits, so half a stack stays half.
        assert_eq!(voice_gain(&unison(4, 12.0), 0.5), 0.125);
        // And a negative weight is still nothing rather than a phase flip.
        assert_eq!(
            voice_gain(
                &Osc {
                    gain: -1.0,
                    ..unison(4, 12.0)
                },
                1.0
            ),
            0.0
        );
    }

    /// Width fans the voices out the way spread fans their pitches: evenly,
    /// symmetrically, lowest on the left, and at full width the outermost
    /// pair hard over.
    #[test]
    fn a_wide_unison_fans_out_across_the_field() {
        let wide = Osc {
            width: 1.0,
            ..unison(5, 20.0)
        };
        let pans: Vec<f32> = (0..5).map(|v| voice_pan(&wide, v)).collect();
        assert_eq!(pans, vec![-1.0, -0.5, 0.0, 0.5, 1.0]);
        let half = Osc {
            width: 0.5,
            ..unison(2, 20.0)
        };
        assert_eq!([voice_pan(&half, 0), voice_pan(&half, 1)], [-0.5, 0.5]);
        // Past the edge is the edge, and nonsense is the centre.
        let over = Osc {
            width: 3.0,
            ..unison(3, 20.0)
        };
        assert_eq!(voice_pan(&over, 0), -1.0);
        for absurd in [f32::NAN, f32::INFINITY, -1.0] {
            let odd = Osc {
                width: absurd,
                ..unison(3, 20.0)
            };
            assert_eq!(voice_pan(&odd, 0), 0.0, "width {absurd} moved a voice");
        }
    }

    /// The default keeps every voice centred, and so does a single voice at
    /// any width — there is nothing to fan out.
    #[test]
    fn an_unspread_or_lone_voice_stays_centred() {
        for voice in 0..MAX_VOICES {
            assert_eq!(voice_pan(&unison(MAX_VOICES, 20.0), voice), 0.0);
        }
        let lone = Osc {
            width: 1.0,
            ..unison(1, 20.0)
        };
        assert_eq!(voice_pan(&lone, 0), 0.0);
    }

    /// A count the validator refuses can still arrive by calling the renderer
    /// directly, and an oscillator sounding no voices at all would be a silent
    /// stack that passed every gain check — and a divide by zero.
    #[test]
    fn an_oscillator_with_no_voices_still_sounds_once() {
        let none = unison(0, 12.0);
        assert_eq!(voices(&none), 1);
        assert_eq!(voice_gain(&none, 1.0), 1.0);
        assert_eq!(voice_detune(&none, 0), 0.0);
    }
}
