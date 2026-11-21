//! How long a rendered note is, and why it is longer than the note.

use crate::common::{opts, peak, refusal, render, saw_patch};
use scorsese_zimmer::patch::{Fx, Source};
use scorsese_zimmer::{SAMPLE_RATE, SynthError};

/// Samples in `seconds` of audio, the way the renderer counts them.
fn samples(seconds: f32) -> usize {
    (seconds * SAMPLE_RATE as f32).ceil() as usize
}

#[test]
fn the_buffer_is_the_gate_plus_the_release_tail() {
    let mut patch = saw_patch();
    patch.amp.r = 0.25;
    let buf = render(&patch, 60.0, &opts(0.5));
    assert_eq!(buf.len(), samples(0.75), "0.5 s gate + 0.25 s release");
    assert!(
        buf[22_100..].iter().any(|sample| sample.abs() > 0.1),
        "the tail rings"
    );
    assert!(buf[buf.len() - 1].abs() < 1e-3, "and ends in silence");
}

/// An echo cut off mid-repeat is a click, not an echo — so the buffer grows to
/// hold whatever the chain needs.
#[test]
fn an_fx_chain_lengthens_the_buffer_so_its_tail_survives() {
    let mut patch = saw_patch();
    patch.fx = vec![Fx::Reverb {
        size: 0.8,
        damp: 0.5,
        mix: 0.4,
        predelay: 0.0,
    }];
    let dry = render(&saw_patch(), 60.0, &opts(0.2));
    let wet = render(&patch, 60.0, &opts(0.2));
    assert!(wet.len() > dry.len(), "the reverb tail needs room");
    assert!(
        wet[dry.len()..].iter().any(|sample| sample.abs() > 1e-4),
        "and rings there"
    );
}

/// The other half of that rule. A waveshaper has no memory, so it colours the
/// note without adding a millisecond to it — and a note that grew would mean
/// state had crept into an effect that is a per-sample function.
#[test]
fn a_saturator_colours_the_note_without_lengthening_it() {
    let mut patch = saw_patch();
    patch.fx = vec![Fx::Saturate {
        drive: 4.0,
        mix: 1.0,
    }];
    let dry = render(&saw_patch(), 60.0, &opts(0.2));
    let driven = render(&patch, 60.0, &opts(0.2));
    assert_eq!(driven.len(), dry.len(), "no tail to make room for");
    assert_ne!(driven, dry, "but the signal itself is not the one it was");
}

#[test]
fn a_note_with_no_length_is_refused_by_the_number_that_is_wrong() {
    for duration in [0.0, -1.0] {
        assert_eq!(
            refusal(&saw_patch(), 60.0, &opts(duration)),
            SynthError::BadDuration { duration }
        );
    }
    assert!(matches!(
        refusal(&saw_patch(), 60.0, &opts(f32::NAN)),
        SynthError::BadDuration { .. }
    ));
}

/// A typo in a duration should fail loudly rather than allocate for an hour of
/// audio — and a note past the ceiling is refused, never cut off at it with a
/// click (#1093). The tail counts: a 58 s drone through a reverb is longer
/// than 58 s.
#[test]
fn a_note_past_the_ceiling_is_refused_tail_included() {
    let too_long = refusal(&saw_patch(), 60.0, &opts(70.0));
    assert!(
        matches!(too_long, SynthError::NoteTooLong { at: None, held, limit, .. }
            if held == 70.0 && limit == 60.0),
        "{too_long:?}"
    );
    let mut drone = saw_patch();
    drone.fx = vec![Fx::Reverb {
        size: 0.9,
        damp: 0.5,
        mix: 0.4,
        predelay: 0.0,
    }];
    let rings_past = refusal(&drone, 60.0, &opts(58.0));
    assert!(
        matches!(rings_past, SynthError::NoteTooLong { tail, .. } if tail > 2.0),
        "{rings_past:?}"
    );
    assert!(
        rings_past.to_string().contains("at most 60 s"),
        "{rings_past}"
    );
    assert!(render(&saw_patch(), 60.0, &opts(30.0)).len() >= samples(30.0));
}

#[test]
fn an_invalid_patch_is_refused_before_any_sample_is_written() {
    let mut broken = saw_patch();
    broken.source = Source::OscStack { oscs: vec![] };
    assert_eq!(
        refusal(&broken, 60.0, &opts(0.5)),
        SynthError::EmptyOscStack
    );
}

#[test]
fn velocity_scales_the_note_linearly() {
    let mut soft = opts(0.2);
    soft.velocity = 0.25;
    let quiet = render(&saw_patch(), 60.0, &soft);
    let loud = render(&saw_patch(), 60.0, &opts(0.2));
    assert!((peak(&loud) * 0.25 - peak(&quiet)).abs() < 0.01);
}
