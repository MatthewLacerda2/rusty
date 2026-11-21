//! A chord's voices, held to the notes its name and fields promise.

use super::*;

mod voicing;

/// A chord to voice, at the octave the document gave it.
fn chord(name: &str, oct: Option<i32>) -> Chord {
    Chord {
        track: "keys".to_owned(),
        chord: Voicing::Name(name.to_owned()),
        oct,
        start: 0.0,
        dur: 1.0,
        vel: 0.8,
        articulation: None,
        arp: None,
        div: None,
        gate: None,
        inv: None,
        voicing: None,
    }
}

/// The MIDI numbers a chord comes out as.
fn voiced(chord: &Chord) -> Result<Vec<f32>, SynthError> {
    let mut out = Vec::new();
    chord.voice_into(&mut Led::default(), &mut out)?;
    out.iter().map(|note| note.note.to_midi()).collect()
}

/// The voicing rule, stated as numbers: close position, root upward.
#[test]
fn a_name_voices_close_position_from_its_root() {
    // D3 F3 A3 C4 — the chord the module doc promises.
    assert_eq!(
        voiced(&chord("Dm7", Some(3))).expect("Dm7 voices"),
        vec![50.0, 53.0, 57.0, 60.0]
    );
    // C4 E4 G4, and the same triad over a slash bass under it.
    assert_eq!(
        voiced(&chord("C", Some(4))).expect("C voices"),
        vec![60.0, 64.0, 67.0]
    );
    assert_eq!(
        voiced(&chord("C/G", Some(4))).expect("C/G voices"),
        vec![55.0, 60.0, 64.0, 67.0]
    );
}

/// An absent octave is a documented default rather than a guess, and it is
/// the one the field's own doc names.
#[test]
fn an_absent_octave_roots_the_chord_at_three() {
    assert_eq!(
        voiced(&chord("Dm7", None)).expect("voices"),
        voiced(&chord("Dm7", Some(DEFAULT_OCT))).expect("voices")
    );
    assert_eq!(DEFAULT_OCT, 3, "the doc on `oct` says three");
}

/// Every voice keeps the chord's timing and force — that shared line is
/// the whole point of writing it as one entry.
#[test]
fn every_voice_carries_the_chord_it_came_from() {
    let mut out = Vec::new();
    chord("Dm7", Some(3))
        .voice_into(&mut Led::default(), &mut out)
        .expect("voices");
    assert_eq!(out.len(), 4, "a seventh chord is four voices");
    assert!(
        out.iter().all(|note| note.track == "keys"
            && note.start == 0.0
            && note.dur == 1.0
            && note.vel == 0.8),
        "a voice disagreed with the chord that wrote it"
    );
}

/// Refused rather than clamped: the octave is in the same entry, so this
/// is a document that can be corrected rather than a pattern meeting a
/// transform written somewhere else.
#[test]
fn a_chord_voiced_off_the_keyboard_is_refused() {
    assert!(matches!(
        voiced(&chord("Cmaj9", Some(9))),
        Err(SynthError::ChordOutOfRange { midi, .. }) if midi > 127
    ));
    assert!(matches!(
        voiced(&chord("C/G", Some(-1))),
        Err(SynthError::ChordOutOfRange { midi, .. }) if midi < 0
    ));
}

/// The escape hatch, and the two ways of writing it that say nothing.
#[test]
fn spelled_pitches_are_the_chord_and_take_no_octave() {
    let spelled = |oct, pitches: Vec<Pitch>| Chord {
        chord: Voicing::Pitches(pitches),
        oct,
        ..chord("unused", None)
    };
    let both_spellings = vec![Pitch::Name("D3".to_owned()), Pitch::Midi(65.0)];
    assert_eq!(
        voiced(&spelled(None, both_spellings)).expect("voices"),
        vec![50.0, 65.0]
    );
    assert!(matches!(
        voiced(&spelled(Some(3), vec![Pitch::Midi(60.0)])),
        Err(SynthError::SpelledChordOctave { .. })
    ));
    assert!(matches!(
        voiced(&spelled(None, vec![])),
        Err(SynthError::EmptyChord { .. })
    ));
}
