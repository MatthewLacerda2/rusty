//! The lift, and everything that is refused for want of a key.
//!
//! A chromatic `transpose` moves the music to another key; `transpose_degrees`
//! moves it *within* the one it is in. The tests below hold that difference to
//! actual samples rather than to arithmetic, and hold the four refusals to the
//! errors they are supposed to raise.

use super::setup::{degree, lifted, playing, render, triad};
use scorsese_zimmer::song::Degree;
use scorsese_zimmer::{Song, SynthError};

/// The lift: up one step **within** the key, which moves the three notes of
/// the triad by different numbers of semitones — the thing one chromatic
/// number cannot express, and the reason this field exists.
#[test]
fn a_diatonic_transpose_moves_within_the_key() {
    let mut lift = playing(Some("D minor"), triad(["D4", "E4", "F4"]));
    lifted(&mut lift, None, Some(1));
    let by_hand = playing(Some("D minor"), triad(["E4", "F4", "G4"]));
    assert_eq!(render(&lift), render(&by_hand));

    // And the chromatic transpose that is nearest to it is a different piece:
    // two semitones takes the F to F#, which is out of the key.
    let mut chromatic = playing(Some("D minor"), triad(["D4", "E4", "F4"]));
    lifted(&mut chromatic, Some(2.0), None);
    assert_ne!(render(&chromatic), render(&lift));
}

/// Everything a key is needed for is refused when the song declares none, and
/// the two lifts are refused together — see `transpose_degrees`.
#[test]
fn what_needs_a_key_is_refused_rather_than_guessed() {
    let refusal = |song: &Song| song.validate().expect_err("the song is refused");
    let keyless = playing(None, vec![degree(Degree::Plain(5), 4, 0.0)]);
    assert!(matches!(
        refusal(&keyless),
        SynthError::DegreeWithoutKey { .. }
    ));

    let mut no_key = playing(None, triad(["D4", "E4", "F4"]));
    lifted(&mut no_key, None, Some(1));
    assert!(matches!(
        refusal(&no_key),
        SynthError::DiatonicWithoutKey { .. }
    ));

    let mut both = playing(Some("D minor"), triad(["D4", "E4", "F4"]));
    lifted(&mut both, Some(12.0), Some(1));
    assert!(matches!(refusal(&both), SynthError::TwoTransposes { .. }));

    let unreadable = playing(Some("D harmonic minor"), triad(["D4", "E4", "F4"]));
    assert!(matches!(refusal(&unreadable), SynthError::BadKey { .. }));
}

/// Absolute names are untouched by a key being declared — including a note
/// that is *not* in it, because a deliberate accidental is a real thing.
#[test]
fn declaring_a_key_changes_nothing_a_song_already_said() {
    let notes = triad(["C#4", "F4", "A4"]);
    assert_eq!(
        render(&playing(Some("D minor"), notes.clone())),
        render(&playing(None, notes))
    );
}
