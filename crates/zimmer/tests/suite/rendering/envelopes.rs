//! What the two envelope shapes do to the samples: a bend that changes how a note dies, and a sweep
//! that changes what pitch it starts on.

use crate::common::{adsr, opts, peak, render, rising_crossings, saw_patch};
use scorsese_zimmer::patch::{Adsr, Lfo, LfoTarget, Patch, PitchEnv};

const WINDOW: usize = 4_410;

const STRIKE: usize = 338;

const HALF_WOBBLE: usize = 3_675;

const SHARP_HALF: usize = 29_400;

const FLAT_HALF: usize = SHARP_HALF + HALF_WOBBLE;

fn decaying(curve: f32) -> Patch {
    Patch {
        amp: Adsr {
            curve,
            ..adsr(0.0, 1.0, 0.0, 0.0)
        },
        ..saw_patch()
    }
}

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

#[test]
fn a_curved_decay_is_further_down_than_a_straight_one() {
    let line = render(&decaying(0.0), 60.0, &opts(1.0));
    let bent = render(&decaying(4.0), 60.0, &opts(1.0));

    let (line_mid, bent_mid) = (halfway(&line), halfway(&bent));
    assert!(
        bent_mid < line_mid * 0.5,
        "halfway down: {line_mid} vs {bent_mid}"
    );

    let (line_hit, bent_hit) = (peak(&line[..STRIKE]), peak(&bent[..STRIKE]));
    assert!(
        (bent_hit - line_hit).abs() < line_hit * 0.03,
        "but the strike itself is as loud as it was: {line_hit} vs {bent_hit}"
    );
}

#[test]
fn the_opening_cycles_already_hold_the_loudest_sample_of_the_note() {
    for curve in [0.0, 4.0] {
        let buf = render(&decaying(curve), 60.0, &opts(1.0));
        assert_eq!(peak(&buf[..STRIKE]), peak(&buf), "curve {curve}");
    }
}

#[test]
fn a_negative_curve_holds_its_level_instead_of_shedding_it() {
    let line = render(&decaying(0.0), 60.0, &opts(1.0));
    let held = render(&decaying(-4.0), 60.0, &opts(1.0));
    assert!(
        halfway(&held) > halfway(&line) * 1.5,
        "halfway down: {} vs {}",
        halfway(&held),
        halfway(&line)
    );
}

fn halfway(buf: &[f32]) -> f32 {
    peak(&buf[22_050..22_050 + WINDOW])
}

#[test]
fn no_curve_renders_silence_or_a_non_finite_sample() {
    for curve in [-8.0, -3.0, 0.0, 3.0, 8.0] {
        let buf = render(&decaying(curve), 60.0, &opts(1.0));
        assert!(peak(&buf[..WINDOW]) > 0.5, "curve {curve} rendered nothing");
        assert!(
            buf.iter().all(|sample| sample.is_finite()),
            "curve {curve} went non-finite"
        );
    }
}
