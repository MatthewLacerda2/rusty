//! The LFO as a document: its shapes, its onset, a rate in beats and a second
//! one (#1772) — what is written and what is refused. `lfo_played` has what a
//! note does with them, and `core::lfo` pins the curves by the number.

use crate::common::{minimal, opts, osc, refusal, saw_patch};
use scorsese_zimmer::patch::{Lfo, LfoShape, LfoTarget, Source, Wave};

use scorsese_zimmer::{Patch, SynthError};

fn vibrato() -> Lfo {
    Lfo::new(5.0, 0.5, LfoTarget::Pitch)
}

fn with(lfo: Lfo) -> Patch {
    Patch {
        lfo: Some(lfo),
        ..saw_patch()
    }
}

fn on_the_beat() -> Lfo {
    Lfo {
        beats: Some(0.5),
        ..Lfo::new(0.0, 0.8, LfoTarget::Amp)
    }
}

/// A plain LFO is written as it always was; every new field is left out at
/// its default, and so is an absent `lfo2`, so no recipe's bytes move.
#[test]
fn an_lfo_at_its_defaults_writes_only_what_it_always_did() {
    let json = with(vibrato()).to_json().expect("serialise");
    for absent in ["lfo2", "beats", "shape", "delay", "fade"] {
        assert!(!json.contains(absent), "`{absent}` is in {json}");
    }
    let json = with(on_the_beat()).to_json().expect("serialise");
    assert!(!json.contains("rate"), "a beats LFO writes no rate: {json}");
    let back = Patch::from_json(&json).expect("parse");
    assert_eq!(back.lfo, Some(on_the_beat()));
    let shaped = Lfo {
        shape: LfoShape::SawDown,
        ..vibrato()
    };
    let json = with(shaped).to_json().expect("serialise");
    assert!(json.contains("saw_down"), "the shape is lost from {json}");
    assert_eq!(Patch::from_json(&json).expect("parse").lfo, Some(shaped));
}

#[test]
fn an_lfo_with_two_speeds_or_none_is_refused() {
    let both = Lfo {
        rate: 2.0,
        ..on_the_beat()
    };
    let none = Lfo::new(0.0, 0.5, LfoTarget::Pitch);
    for (lfo, want) in [
        (
            both,
            SynthError::LfoTwoRates {
                rate: 2.0,
                beats: 0.5,
            },
        ),
        (none, SynthError::LfoNoRate),
    ] {
        assert_eq!(with(lfo).validate_in_song(), Err(want));
    }
}

/// A patch played on its own has no tempo, so `beats` is refused there and
/// accepted in a song; a cycle of no beats is refused everywhere.
#[test]
fn beats_need_a_song_and_a_positive_length() {
    let patch = with(on_the_beat());
    assert_eq!(
        refusal(&patch, 60.0, &opts(0.2)),
        SynthError::LfoBeatsWithoutTempo { beats: 0.5 }
    );
    assert_eq!(patch.validate_in_song(), Ok(()));
    for beats in [0.0, -1.0, f32::INFINITY] {
        let lfo = Lfo {
            beats: Some(beats),
            ..on_the_beat()
        };
        let error = with(lfo).validate_in_song().expect_err("refused");
        assert!(matches!(error, SynthError::BadLfoBeats { .. }), "{error:?}");
    }
}

#[test]
fn a_negative_onset_is_refused() {
    for (lfo, field) in [
        (
            Lfo {
                delay: -0.1,
                ..vibrato()
            },
            "delay",
        ),
        (
            Lfo {
                fade: f32::NAN,
                ..vibrato()
            },
            "fade",
        ),
    ] {
        let error = with(lfo).validate().expect_err("refused");
        assert!(
            matches!(error, SynthError::BadLfoOnset { field: f, .. } if f == field),
            "{error:?}"
        );
    }
}

/// The second LFO is held to the same rules as the first, its pulse-width
/// target included.
#[test]
fn the_second_lfo_is_checked_like_the_first() {
    let patch = Patch {
        lfo2: Some(Lfo::new(1.0, 0.2, LfoTarget::PulseWidth)),
        ..saw_patch()
    };
    assert_eq!(patch.validate(), Err(SynthError::NoPulseToModulate));
    let squared = Patch {
        lfo2: patch.lfo2,
        ..minimal(Source::OscStack {
            oscs: vec![osc(Wave::Square, 0.0, 0)],
        })
    };
    assert_eq!(squared.validate(), Ok(()));
}
