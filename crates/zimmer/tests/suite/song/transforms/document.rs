//! Writing a transformed entry down, and what it is refused for.

use crate::common::songs::song;
use scorsese_zimmer::song::{ArrangementEntry, Play};
use scorsese_zimmer::{Song, SynthError};

use super::setup::play;

/// Untagged serde has to route a JSON string to the name arm and an object to
/// the long form. If those raced, a saved song would change on being reloaded —
/// and the short form staying short is what keeps the format cheap to write.
#[test]
fn a_mixed_arrangement_round_trips_with_the_short_form_still_short() {
    let mut mixed = song();
    mixed.arrangement = vec![
        "verse".into(),
        ArrangementEntry::from(Play {
            transpose: Some(12.0),
            vel_scale: Some(0.7),
            tracks: Some(vec!["bass".to_owned()]),
            ..play("verse")
        }),
    ];

    let json = mixed.to_json().expect("serialise");
    assert_eq!(Song::from_json(&json).expect("deserialise"), mixed);
    assert!(
        json.contains("\"verse\","),
        "the bare name must survive as a bare name, not grow into an object:\n{json}"
    );
}

/// The same typo as an arrangement naming no real pattern, with the same
/// consequence — an instrument that silently never plays.
#[test]
fn a_tracks_filter_naming_no_real_track_is_refused() {
    let mut typo = song();
    typo.arrangement = vec![ArrangementEntry::from(Play {
        tracks: Some(vec!["basss".to_owned()]),
        ..play("verse")
    })];
    assert_eq!(
        typo.validate(),
        Err(SynthError::UnknownTrackFilter {
            pattern: "verse".to_owned(),
            track: "basss".to_owned(),
        })
    );
}

/// Negative would be a phase inversion by another name, which is not what
/// "quieter" means.
#[test]
fn a_velocity_scale_below_zero_is_refused() {
    let mut inverted = song();
    inverted.arrangement = vec![ArrangementEntry::from(Play {
        vel_scale: Some(-1.0),
        ..play("verse")
    })];
    assert_eq!(
        inverted.validate(),
        Err(SynthError::BadVelocityScale {
            pattern: "verse".to_owned(),
            scale: -1.0,
        })
    );
}

/// A NaN would reach the mix as a whole song of silence, a long way from the
/// field that caused it.
#[test]
fn a_transpose_that_is_not_a_number_is_refused() {
    let mut nonsense = song();
    nonsense.arrangement = vec![ArrangementEntry::from(Play {
        transpose: Some(f32::NAN),
        ..play("verse")
    })];
    assert!(matches!(
        nonsense.validate(),
        Err(SynthError::BadTranspose { .. })
    ));
}

/// The long form is held to the same unknown-pattern check the bare name is:
/// a typo in `pattern` must not become silence in the middle of a piece.
#[test]
fn a_long_form_entry_naming_no_real_pattern_is_refused() {
    let mut typo = song();
    typo.arrangement = vec![play("chorus").into()];
    assert_eq!(
        typo.validate(),
        Err(SynthError::UnknownPattern {
            pattern: "chorus".to_owned()
        })
    );
}
