//! What the two envelope shapes do to the samples: a sweep that changes what pitch a note starts
//! on here, and a bend that changes how it dies in `curves`.

use crate::common::{adsr, opts, render, rising_crossings, saw_patch};
use rusty_zimmer::patch::{Lfo, LfoTarget, PitchEnv};

mod curves;

/// Samples, a tenth of a second: whole cycles of A2, so a count is pitch, not phase.
const WINDOW: usize = 4_410;

/// Half a cycle of the 6 Hz vibrato: the sine starts at zero, so sharp, then flat.
const HALF_WOBBLE: usize = 3_675;

/// The fourth vibrato cycle, 4 × 7_350: past the sweep.
const SHARP_HALF: usize = 29_400;

const FLAT_HALF: usize = SHARP_HALF + HALF_WOBBLE;

fn sweep(semitones: f32, d: f32) -> PitchEnv {
    PitchEnv {
        semitones,
        adsr: adsr(0.0, d, 0.0, 0.0),
    }
}

fn hz(buf: &[f32], from: usize) -> usize {
    rising_crossings(&buf[from..from + WINDOW])
}

#[test]
fn a_pitch_envelope_starts_the_note_high_and_lands_it_on_the_played_pitch() {
    let mut patch = saw_patch();
    patch.pitch_env = Some(sweep(12.0, 0.5));
    let swept = render(&patch, 45.0, &opts(1.0));
    let plain = render(&saw_patch(), 45.0, &opts(1.0));

    assert!(
        hz(&swept, 0) > hz(&plain, 0) + 4,
        "the sweep starts high: {} vs {}",
        hz(&swept, 0),
        hz(&plain, 0)
    );
    // Within one crossing: a window that is not a whole number of periods counts pitch to ±1.
    assert!(
        hz(&swept, 39_690).abs_diff(hz(&plain, 39_690)) <= 1,
        "and has arrived on the note by the end: {} against {}",
        hz(&swept, 39_690),
        hz(&plain, 39_690)
    );
}

#[test]
fn a_negative_sweep_starts_the_note_low_instead() {
    let mut patch = saw_patch();
    patch.pitch_env = Some(sweep(-12.0, 0.5));
    let swept = render(&patch, 45.0, &opts(1.0));
    let plain = render(&saw_patch(), 45.0, &opts(1.0));
    assert!(
        hz(&swept, 0) < hz(&plain, 0),
        "the sweep starts low: {} vs {}",
        hz(&swept, 0),
        hz(&plain, 0)
    );
}

#[test]
fn a_sweep_and_a_vibrato_are_both_audible_on_one_note() {
    let mut patch = saw_patch();
    patch.pitch_env = Some(sweep(12.0, 0.5));
    patch.lfo = Some(Lfo::new(6.0, 7.0, LfoTarget::Pitch));
    let both = render(&patch, 45.0, &opts(1.0));

    let mut swept_only = saw_patch();
    swept_only.pitch_env = patch.pitch_env;
    let swept = render(&swept_only, 45.0, &opts(1.0));

    assert!(hz(&both, 0) > hz(&swept, 39_690), "the sweep still sweeps");

    let (sharp, flat) = (wobble(&both, SHARP_HALF), wobble(&both, FLAT_HALF));
    assert!(
        sharp > flat + 2,
        "and the vibrato is still wobbling once the sweep has settled: \
         {sharp} against {flat}"
    );

    let steady = (wobble(&swept, SHARP_HALF), wobble(&swept, FLAT_HALF));
    assert!(
        steady.0.abs_diff(steady.1) <= 1,
        "while the sweep alone has settled onto one pitch: {steady:?}"
    );
}

fn wobble(buf: &[f32], from: usize) -> usize {
    rising_crossings(&buf[from..from + HALF_WOBBLE])
}
