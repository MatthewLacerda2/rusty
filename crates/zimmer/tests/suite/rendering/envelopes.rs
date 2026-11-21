//! What the two envelope shapes do to the samples: a bend that changes how a
//! note dies, and a sweep that changes what pitch it starts on.
//!
//! Asked of the rendered buffer rather than of the envelope, which is the half
//! `core::env`'s own tests cannot answer — a shaping function that is perfect
//! and never reaches the signal is still a stage that does nothing.

use crate::common::{adsr, opts, peak, render, rising_crossings, saw_patch};
use scorsese_zimmer::patch::{Adsr, Lfo, LfoTarget, Patch, PitchEnv};

/// A tenth of a second at the render rate: the window every measurement below
/// is taken over, long enough to hold ten cycles of the lowest note used.
///
/// **A window shorter than one cycle measures the wrong thing.** An
/// oscillator's start phase is drawn from its note's seed, so a prefix of a
/// buffer begins at an arbitrary point on the waveform and its loudest sample
/// says where in the ramp the note happened to start rather than how loud the
/// note is. Every window here holds whole cycles for that reason, and
/// [`STRIKE`] is the shortest of them.
const WINDOW: usize = 4_410;

/// Two cycles of C4 — `2 × 44_100 / 261.63`, rounded up. A window whose
/// loudest sample is a statement about amplitude rather than about phase: a saw
/// sweeps its whole range once per cycle, so wherever the phase began an
/// extreme falls inside this many samples.
///
/// Two and not one, though one is the length the argument needs: a cycle here
/// is 168.6 samples, so a window rounded to it leaves an extreme that lands on
/// the boundary just outside. The second cycle costs nothing — the envelope
/// only ever falls, so the extreme in the first is the larger of the two and
/// the reading is the same either way.
const STRIKE: usize = 338;

/// Half a cycle of the 6 Hz vibrato below: `44_100 / 6 / 2`. The LFO is a sine
/// started at zero, so the first half of each of its cycles bends the pitch
/// entirely sharp and the second entirely flat — which is what lets a pitch
/// read over one half be compared against the next.
const HALF_WOBBLE: usize = 3_675;

/// The fourth whole vibrato cycle, `4 × 7_350`. Well past the half second the
/// sweep below takes to settle, so what moves the pitch there is the LFO and
/// nothing else.
const SHARP_HALF: usize = 29_400;

/// The flat half of that same cycle.
const FLAT_HALF: usize = SHARP_HALF + HALF_WOBBLE;

/// A saw whose level decays over a whole second, straight or bent. Nothing
/// else moves, so a difference in the samples is the curve and only the curve.
fn decaying(curve: f32) -> Patch {
    Patch {
        amp: Adsr {
            curve,
            ..adsr(0.0, 1.0, 0.0, 0.0)
        },
        ..saw_patch()
    }
}

/// A sweep of `semitones` arriving on the played note over `d` seconds.
fn sweep(semitones: f32, d: f32) -> PitchEnv {
    PitchEnv {
        semitones,
        adsr: adsr(0.0, d, 0.0, 0.0),
    }
}

/// The pitch a window is at, as rising zero crossings — for a saw this *is*
/// its frequency, so no FFT is needed to say the note moved.
fn hz(buf: &[f32], from: usize) -> usize {
    rising_crossings(&buf[from..from + WINDOW])
}

/// A downward sweep starts the note above the pitch it was played at and
/// leaves it there — the whole gesture, measured as pitch rather than as
/// "something changed".
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
    // Within one crossing, not equal to it: a count over a fixed window
    // resolves a pitch to ±1, because the window is not a whole number of
    // periods and the two notes reach the same frequency at different phases.
    // A sweep that had *not* settled would be an octave out and so nine
    // crossings out, which is nowhere near the slack this allows.
    assert!(
        hz(&swept, 39_690).abs_diff(hz(&plain, 39_690)) <= 1,
        "and has arrived on the note by the end: {} against {}",
        hz(&swept, 39_690),
        hz(&plain, 39_690)
    );
}

/// Negative goes the other way. Stated separately from the positive case
/// because a sign that was ignored would pass a test that only asked whether
/// the pitch moved.
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

/// A sweep and a vibrato are both audible at once. The sum happens in
/// semitones inside one track, so a version where the last modulator written
/// won would lose one of these two assertions.
///
/// The wobble is read as the *sharp* half of a vibrato cycle against the
/// *flat* half that follows it, rather than as one pitch count differing from
/// another note's. A vibrato is a pitch that moves within the note, so that is
/// the thing to measure; counting crossings across a whole window instead
/// averages the wobble away and then asks whether the average happened to land
/// somewhere else, which for some start phases it does not.
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

/// The pitch over half a vibrato cycle starting at `from`, as rising zero
/// crossings.
fn wobble(buf: &[f32], from: usize) -> usize {
    rising_crossings(&buf[from..from + HALF_WOBBLE])
}

/// The point of curving at all: partway through, a bent decay has already shed
/// what a straight one is still holding on to — while the strike that started
/// them both is untouched.
///
/// The two tolerances are far apart on purpose. Halfway down the levels differ
/// by a factor of two; at the strike they may differ only by what a curve
/// legitimately sheds before the note peaks, which within the opening cycle at
/// `curve = 4` is a little over one percent. The effect under test and the
/// slack allowed against it are more than an order of magnitude apart, so the
/// slack cannot swallow it.
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

/// What makes the strike above readable from so short a window: on an envelope
/// that only ever falls, the loudest sample of the whole note is in its opening
/// cycle. So [`STRIKE`] is not a sample of the note, it *is* the note's peak —
/// and if that ever stops holding, this says so rather than letting the
/// measurement quietly go back to reading phase.
#[test]
fn the_opening_cycles_already_hold_the_loudest_sample_of_the_note() {
    for curve in [0.0, 4.0] {
        let buf = render(&decaying(curve), 60.0, &opts(1.0));
        assert_eq!(peak(&buf[..STRIKE]), peak(&buf), "curve {curve}");
    }
}

/// And a negative curve is the other way about — it holds on to its level and
/// then goes, which is what makes it a swell rather than a decay. Stated as
/// its own case because a bend that ignored the sign would pass the test above
/// and still be wrong here.
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

/// How loud a one-second note still is at its halfway mark, over a whole
/// number of cycles so the answer is amplitude and not phase.
fn halfway(buf: &[f32]) -> f32 {
    peak(&buf[22_050..22_050 + WINDOW])
}

/// No bend, either way, may turn the amp stage into silence or into a buffer
/// with a `NaN` in it. That a curve lands exactly on its destination is
/// `core::env`'s to prove; this is the guard that the stage still renders a
/// note at all.
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
