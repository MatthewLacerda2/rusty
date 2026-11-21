//! The document: what a curve looks like on the page, and what it costs a song
//! that does not use one.

use scorsese_zimmer::song::{Easing, Param, Song};

use super::setup::{at, curve, eased, held};
use crate::common::songs::song;

/// A song that automates nothing writes nothing about it.
///
/// This is not tidiness. A bake is addressed by a hash of the recipe's bytes,
/// so a serialiser that started emitting `"automation": []` would invalidate
/// every cached bake in every project at once, for no change in the audio.
#[test]
fn a_song_that_automates_nothing_writes_nothing_about_it() {
    let written = song().to_json().expect("the fixture serialises");
    assert!(
        !written.contains("automation"),
        "an empty list is not a field: {written}"
    );
}

/// And the same for the one default a point has: a point that travels at a
/// constant rate does not say so.
#[test]
fn a_linear_point_does_not_write_its_easing() {
    let mut straight = held(0.5);
    straight.automation = vec![curve(Param::Gain, vec![at(0.0, 0.2), at(8.0, 0.9)])];
    let written = straight.to_json().expect("it serialises");
    assert!(!written.contains("easing"), "{written}");

    let mut bent = held(0.5);
    bent.automation = vec![curve(
        Param::Gain,
        vec![eased(0.0, 0.2, Easing::EaseInOut), at(8.0, 0.9)],
    )];
    let written = bent.to_json().expect("it serialises");
    assert!(written.contains("\"easing\": \"ease_in_out\""), "{written}");
}

/// Round-trip: what is read back is what was written, curves and all.
#[test]
fn a_song_with_curves_round_trips() {
    let mut written = held(0.5);
    written.automation = vec![
        curve(
            Param::Cutoff,
            vec![at(0.0, 300.0), eased(32.0, 6000.0, Easing::EaseIn)],
        ),
        curve(
            Param::Pan,
            vec![eased(0.0, -0.5, Easing::Hold), at(16.0, 0.5)],
        ),
    ];
    let json = written.to_json().expect("it serialises");
    assert_eq!(
        Song::from_json(&json).expect("and parses back"),
        written,
        "a curve survives the page it is written on"
    );
}

/// The parameter names, as a recipe spells them — the closed list an agent has
/// to be able to guess right the first time.
#[test]
fn the_parameters_are_spelled_the_way_the_document_writes_them() {
    let mut song = held(0.5);
    song.automation = vec![
        curve(Param::Gain, vec![at(0.0, 1.0)]),
        curve(Param::Pan, vec![at(0.0, 0.0)]),
    ];
    let written = song.to_json().expect("it serialises");
    for word in ["\"gain\"", "\"pan\""] {
        assert!(written.contains(word), "{word} is missing from {written}");
    }
    assert_eq!(Param::Cutoff.as_str(), "cutoff");
}
