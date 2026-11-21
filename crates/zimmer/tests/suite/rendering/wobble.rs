//! The cutoff LFO, asked about the cutoff rather than about the buffer being
//! different from some other buffer.
//!
//! "The samples changed" is what a wobble shares with every other way of
//! getting the arithmetic wrong — a wrong depth, an inverted phase, or a
//! wobble applied to a patch that asked for a vibrato all change the samples
//! too. So each test here pins one property the LFO is supposed to have, at a
//! moment of the note chosen because the sine is at a known place.

use crate::common::{brightness, opts, render, saw_patch};
use scorsese_zimmer::SAMPLE_RATE;
use scorsese_zimmer::patch::{Adsr, Filter, FilterKind, Lfo, LfoTarget, Patch, Slope};

/// A 1 Hz LFO over a one-second note, so the sine is at zero when the note
/// starts, at its crest a quarter of the way through and at its trough three
/// quarters of the way through — three moments a window can be put on.
const RATE: f32 = 1.0;
/// Either side of a named moment, in samples — a twentieth of a second, which
/// is several periods of the note and a twentieth of the LFO's own.
const WINDOW: usize = SAMPLE_RATE as usize / 20;
/// A tenth of that, for the moment where the question is what the sine is
/// doing *right there* rather than over a stretch of the wobble.
const NARROW: usize = WINDOW / 10;

fn wobbling(depth: f32, target: LfoTarget) -> Patch {
    Patch {
        filter: Some(Filter {
            kind: FilterKind::Lowpass,
            cutoff: 500.0,
            slope: Slope::Db12,
            resonance: 0.0,
            env_octaves: 0.0,
            vel_octaves: 0.0,
            key_track: 0.0,
            adsr: Adsr::default(),
        }),
        lfo: Some(Lfo::new(RATE, depth, target)),
        ..saw_patch()
    }
}

/// How bright the note is around `at` seconds, over `half` samples either way.
fn window(buf: &[f32], at: f32, half: usize) -> f32 {
    let centre = (at * SAMPLE_RATE as f32) as usize;
    brightness(&buf[centre - half..centre + half])
}

/// How bright the note is around `at` seconds.
fn around(buf: &[f32], at: f32) -> f32 {
    window(buf, at, WINDOW)
}

/// The crest opens the filter and the trough closes it — in that order. An
/// inverted wobble is the same sweep at the opposite phase, so a measurement
/// taken over the whole note cannot tell the two apart and a measurement taken
/// at a named moment can.
#[test]
fn the_crest_of_the_sine_is_the_bright_half_of_the_wobble() {
    let buf = render(&wobbling(2.0, LfoTarget::Cutoff), 45.0, &opts(1.0));
    let (crest, trough) = (around(&buf, 0.25), around(&buf, 0.75));
    assert!(
        crest > trough * 2.0,
        "the filter opens on the crest: {crest} vs {trough}"
    );
}

/// Where the sine is at zero the wobble contributes nothing, so the note
/// sounds as it would with no LFO at all. That is what makes `depth` a
/// *depth* rather than an offset: a wobble that moved the cutoff even at its
/// own zero would be a detune with a wobble on top of it.
#[test]
fn the_zero_crossing_sounds_like_no_wobble_at_all() {
    let mut still = wobbling(3.0, LfoTarget::Cutoff);
    let wobbled = render(&still, 45.0, &opts(1.0));
    still.lfo = None;
    let steady = render(&still, 45.0, &opts(1.0));
    // Half a second in: the sine is passing through zero on its way down, so a
    // narrow window there is the wobble at its own nothing.
    let (moving, parked) = (window(&wobbled, 0.5, NARROW), window(&steady, 0.5, NARROW));
    assert!(
        (moving - parked).abs() < parked * 0.15,
        "the sine crosses zero and so does the wobble: {moving} vs {parked}"
    );
}

/// A deeper setting is a wider swing. Nothing else about the note changes, so
/// the distance between the crest and the trough is the whole of what `depth`
/// buys.
#[test]
fn a_deeper_setting_swings_the_cutoff_further() {
    let swing = |depth: f32| {
        let buf = render(&wobbling(depth, LfoTarget::Cutoff), 45.0, &opts(1.0));
        around(&buf, 0.25) / around(&buf, 0.75)
    };
    let (shallow, deep) = (swing(0.5), swing(3.0));
    assert!(shallow > 1.0, "even a shallow wobble moves it: {shallow}");
    assert!(deep > shallow * 2.0, "and a deep one further: {deep}");
}

/// An LFO aimed somewhere else leaves the cutoff alone. A tremolo scales the
/// whole note, and `brightness` is a ratio, so a filter that never moved reads
/// the same at the crest as at the trough — which it would not if every LFO
/// reached the filter.
#[test]
fn an_lfo_aimed_elsewhere_does_not_reach_the_filter() {
    let buf = render(&wobbling(0.8, LfoTarget::Amp), 45.0, &opts(1.0));
    let (crest, trough) = (around(&buf, 0.25), around(&buf, 0.75));
    assert!(
        (crest - trough).abs() < trough * 0.1,
        "a tremolo is not a wobble: {crest} vs {trough}"
    );
}
