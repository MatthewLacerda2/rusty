//! How long a baked one-shot's file is (#970): as long as the sound, not as
//! long as the renderer's worst-case allowance for its release and effects.
//!
//! The pop below is the shape that reported it: a tenth of a second of sine
//! falling onto its pitch, through a small room. Its raw render is sized for
//! the room's longest ring and runs past 1.2 s.

use crate::common::{adsr, minimal, opts, osc, render};
use scorsese_zimmer::patch::{Fx, Patch, PitchEnv, Source, Wave};
use scorsese_zimmer::{SAMPLE_RATE, bake_note, wav};

/// A 0.1 s pop with a little reverb on it.
fn pop() -> Patch {
    let mut patch = minimal(Source::OscStack {
        oscs: vec![osc(Wave::Sine, 0.0, 0)],
    });
    patch.amp = adsr(0.001, 0.08, 0.0, 0.03);
    patch.pitch_env = Some(PitchEnv {
        semitones: 12.0,
        adsr: adsr(0.0, 0.04, 0.0, 0.0),
    });
    patch.fx = vec![Fx::Reverb {
        size: 0.25,
        damp: 0.6,
        mix: 0.2,
        predelay: 0.0,
    }];
    patch
}

/// Seconds of the raw, untrimmed render (one channel of it).
fn raw_seconds(patch: &Patch) -> f64 {
    render(patch, 72.0, &opts(0.05)).len() as f64 / f64::from(SAMPLE_RATE)
}

#[test]
fn a_pop_with_reverb_bakes_to_its_sound_not_its_allowance() {
    let bake = bake_note(&pop(), 72.0, &opts(0.05)).expect("the pop bakes");
    let baked = wav::seconds_in(bake.wav.len());
    let raw = raw_seconds(&pop());
    assert!(raw > 1.0, "the allowance is the worst case: {raw:.2} s");
    assert!(
        baked < raw * 0.6,
        "{baked:.2} s baked of {raw:.2} s rendered"
    );
    assert!(baked > 0.3, "the room still rings: {baked:.2} s");
}

/// A dry note was always about as long as its sound; the trim only takes the
/// release's last whisper off it.
#[test]
fn a_dry_note_keeps_its_release() {
    let mut patch = pop();
    patch.fx.clear();
    let bake = bake_note(&patch, 72.0, &opts(0.05)).expect("the dry pop bakes");
    let baked = wav::seconds_in(bake.wav.len());
    assert!(baked > 0.05 && baked <= raw_seconds(&patch), "{baked:.3} s");
}
