//! Writing a transformed entry down, and what it is refused for.

use crate::common::songs::song;
use scorsese_zimmer::song::{ArrangementEntry, Play};
use scorsese_zimmer::{Song, SynthError};

use super::setup::play;

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
