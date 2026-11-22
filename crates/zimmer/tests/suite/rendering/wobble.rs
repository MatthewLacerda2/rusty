//! The cutoff LFO, asked about the cutoff rather than about the buffer being different from some
//! other buffer.

use crate::common::{brightness, opts, render, saw_patch};
use rusty_zimmer::patch::{Adsr, Filter, FilterKind, Lfo, LfoTarget, Patch, Slope};
use rusty_zimmer::SAMPLE_RATE;

/// Hz: over the one-second note the sine is at zero at the start, its crest at a quarter, its
/// trough at three quarters.
const RATE: f32 = 1.0;
/// Samples either side of a moment, a twentieth of a second: several periods of the note.
const WINDOW: usize = SAMPLE_RATE as usize / 20;
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

fn window(buf: &[f32], at: f32, half: usize) -> f32 {
    let centre = (at * SAMPLE_RATE as f32) as usize;
    brightness(&buf[centre - half..centre + half])
}

fn around(buf: &[f32], at: f32) -> f32 {
    window(buf, at, WINDOW)
}

#[test]
fn the_crest_of_the_sine_is_the_bright_half_of_the_wobble() {
    let buf = render(&wobbling(2.0, LfoTarget::Cutoff), 45.0, &opts(1.0));
    let (crest, trough) = (around(&buf, 0.25), around(&buf, 0.75));
    assert!(
        crest > trough * 2.0,
        "the filter opens on the crest: {crest} vs {trough}"
    );
}

#[test]
fn the_zero_crossing_sounds_like_no_wobble_at_all() {
    let mut still = wobbling(3.0, LfoTarget::Cutoff);
    let wobbled = render(&still, 45.0, &opts(1.0));
    still.lfo = None;
    let steady = render(&still, 45.0, &opts(1.0));
    // Half a second in: the sine is passing through zero on its way down, so a narrow window there
    // is the wobble at its own nothing.
    let (moving, parked) = (window(&wobbled, 0.5, NARROW), window(&steady, 0.5, NARROW));
    assert!(
        (moving - parked).abs() < parked * 0.15,
        "the sine crosses zero and so does the wobble: {moving} vs {parked}"
    );
}

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

#[test]
fn an_lfo_aimed_elsewhere_does_not_reach_the_filter() {
    let buf = render(&wobbling(0.8, LfoTarget::Amp), 45.0, &opts(1.0));
    let (crest, trough) = (around(&buf, 0.25), around(&buf, 0.75));
    assert!(
        (crest - trough).abs() < trough * 0.1,
        "a tremolo is not a wobble: {crest} vs {trough}"
    );
}
