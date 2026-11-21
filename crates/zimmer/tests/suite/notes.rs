//! Note names, and the pitch they come out at.

use scorsese_zimmer::{NoteOpts, SynthError, midi_to_freq, parse_note};

/// The note name, or the refusal, without the `Result` ceremony at every call.
fn midi(name: &str) -> f32 {
    parse_note(name).expect("a real note name")
}

#[test]
fn middle_c_and_concert_a_have_the_standard_numbers() {
    assert_eq!(midi("C4"), 60.0);
    assert_eq!(midi("A4"), 69.0);
    assert_eq!(midi("C-1"), 0.0, "the bottom of the MIDI range");
    assert_eq!(midi("G9"), 127.0, "and the top");
}

#[test]
fn accidentals_are_case_insensitive_and_stack() {
    assert_eq!(midi("C#4"), 61.0);
    assert_eq!(midi("db4"), 61.0, "the same key, spelled the other way");
    assert_eq!(midi("cs4"), 61.0, "`s` for sharp, for keyboards without #");
    assert_eq!(midi("Bb3"), 58.0);
    assert_eq!(midi("C##4"), 62.0);
}

#[test]
fn a_malformed_name_says_which_part_was_wrong() {
    assert_eq!(parse_note(""), Err(SynthError::EmptyNoteName));
    assert_eq!(
        parse_note("H4"),
        Err(SynthError::BadNoteLetter {
            name: "H4".to_owned(),
            letter: 'h',
        })
    );
    assert_eq!(
        parse_note("C"),
        Err(SynthError::BadOctave {
            name: "C".to_owned(),
            octave: String::new(),
        })
    );
    assert!(matches!(
        parse_note("C-2"),
        Err(SynthError::NoteOutOfRange { midi: -12, .. })
    ));
}

#[test]
fn equal_temperament_anchors_at_440() {
    assert!((midi_to_freq(69.0) - 440.0).abs() < 1e-3);
    assert!(
        (midi_to_freq(81.0) - 880.0).abs() < 1e-3,
        "an octave up doubles"
    );
    assert!(
        (midi_to_freq(57.0) - 220.0).abs() < 1e-3,
        "an octave down halves"
    );
    assert!(
        (midi_to_freq(60.0) - 261.6256).abs() < 1e-2,
        "middle C, the other number everyone knows"
    );
}

#[test]
fn default_opts_describe_a_short_one_shot() {
    let opts = NoteOpts::default();
    assert_eq!(opts.duration, 0.5);
    assert_eq!(opts.seed, 0);
    assert!(opts.velocity > 0.0 && opts.velocity <= 1.0);
}
