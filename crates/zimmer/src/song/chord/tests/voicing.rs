//! `inv` and `voicing`: a named chord voiced other than in root position.

use super::{chord, voiced};
use crate::error::SynthError;
use crate::song::Pitch;
use crate::song::chord::{Chord, Leading, Led, Voicing};

/// `inv` rotates the voices above the bass and nothing else, and stops at
/// the last inversion.
#[test]
fn an_inversion_turns_the_voices_above_the_bass() {
    let inverted = |name, inv| Chord {
        inv: Some(inv),
        ..chord(name, Some(4))
    };
    // E4 G4 C5, then G4 C5 E5.
    assert_eq!(
        voiced(&inverted("C", 1)).expect("v"),
        vec![64.0, 67.0, 72.0]
    );
    assert_eq!(
        voiced(&inverted("C", 2)).expect("v"),
        vec![67.0, 72.0, 76.0]
    );
    // The G under C/G stays put while the triad turns.
    assert_eq!(
        voiced(&inverted("C/G", 1)).expect("v"),
        vec![55.0, 64.0, 67.0, 72.0]
    );
    assert_eq!(voiced(&inverted("Dm7", 3)).expect("v")[0], 72.0);
    assert!(matches!(
        voiced(&inverted("C", 3)),
        Err(SynthError::BadVoicing { .. })
    ));
    // Root position fits; its second inversion's top voice does not.
    assert!(matches!(
        voiced(&Chord {
            inv: Some(2),
            ..chord("C", Some(9))
        }),
        Err(SynthError::ChordOutOfRange { midi, .. }) if midi > 127
    ));
}

/// `near` reads the last chord on its own track, and is root position
/// when there is none.
#[test]
fn near_leads_from_the_last_chord_on_its_track() {
    let near = |name, track: &str| Chord {
        voicing: Some(Leading::Near),
        track: track.to_owned(),
        ..chord(name, Some(3))
    };
    let mut led = Led::default();
    let mut out = Vec::new();
    near("Dmaj7", "keys")
        .voice_into(&mut led, &mut out)
        .expect("v");
    near("Bm7", "pad")
        .voice_into(&mut led, &mut out)
        .expect("v");
    near("Bm7", "keys")
        .voice_into(&mut led, &mut out)
        .expect("v");
    let midi: Vec<f32> = out
        .iter()
        .map(|n| n.note.to_midi().expect("midi"))
        .collect();
    // Dmaj7 in root position; Bm7 alone on `pad` likewise; Bm7 after
    // Dmaj7 on `keys` keeps D, F# and A and moves C# down to B.
    assert_eq!(
        midi,
        [
            50.0, 54.0, 57.0, 61.0, 59.0, 62.0, 66.0, 69.0, 50.0, 54.0, 57.0, 59.0
        ]
    );
}

/// The combinations that say two things, or nothing, are refused.
#[test]
fn a_voicing_that_cannot_apply_is_refused() {
    let both = Chord {
        inv: Some(1),
        voicing: Some(Leading::Near),
        ..chord("C", None)
    };
    assert!(matches!(voiced(&both), Err(SynthError::BadVoicing { .. })));
    let spelled = Chord {
        chord: Voicing::Pitches(vec![Pitch::Midi(60.0)]),
        voicing: Some(Leading::Near),
        ..chord("unused", None)
    };
    assert!(matches!(
        voiced(&spelled),
        Err(SynthError::BadVoicing { .. })
    ));
}
