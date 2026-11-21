//! What a curve is refused for.
//!
//! One rule underneath all of it: **a curve that moves nothing is refused, not
//! ignored.** A misspelled track, an empty list, a second curve on the same
//! parameter and a sweep on an instrument with no filter are all the same
//! failure — a build the recipe says is there and nothing can hear — and it is
//! the failure that costs an agent a whole iteration to even notice.

use scorsese_zimmer::song::{Automation, InlineOnly, Param, Point};
use scorsese_zimmer::{SynthError, render_song};

use super::setup::{at, curve, filtered, held, voiced};

/// The refusal a song carrying these curves is turned away with.
fn refused(moving: Vec<Automation>) -> SynthError {
    let mut song = held(0.5);
    song.automation = moving;
    song.validate().expect_err("the song is refused")
}

#[test]
fn a_curve_on_a_track_that_does_not_exist_is_refused() {
    let mut wrong = curve(Param::Gain, vec![at(0.0, 1.0)]);
    wrong.track = "paad".to_owned();
    assert!(matches!(
        refused(vec![wrong]),
        SynthError::UnknownAutomationTrack { ref track, param }
            if track == "paad" && param == "gain"
    ));
}

#[test]
fn two_curves_on_one_parameter_are_refused_rather_than_ordered() {
    let twice = vec![
        curve(Param::Gain, vec![at(0.0, 1.0)]),
        curve(Param::Gain, vec![at(0.0, 0.2)]),
    ];
    assert!(matches!(
        refused(twice),
        SynthError::DuplicateAutomation { param, .. } if param == "gain"
    ));
    let both = vec![
        curve(Param::Gain, vec![at(0.0, 1.0)]),
        curve(Param::Pan, vec![at(0.0, 0.2)]),
    ];
    let mut song = held(0.5);
    song.automation = both;
    song.validate()
        .expect("two parameters are two curves, not a clash");
}

#[test]
fn a_curve_with_no_points_is_refused() {
    assert!(matches!(
        refused(vec![curve(Param::Gain, vec![])]),
        SynthError::BadAutomationCurve { why, .. } if why.contains("no points")
    ));
}

#[test]
fn points_that_do_not_ascend_are_refused_rather_than_sorted() {
    for points in [
        vec![at(4.0, 0.1), at(1.0, 0.9)],
        vec![at(1.0, 0.1), at(1.0, 0.9)],
    ] {
        assert!(matches!(
            refused(vec![curve(Param::Gain, points)]),
            SynthError::BadAutomationCurve { why, .. } if why.contains("ascend")
        ));
    }
}

#[test]
fn a_beat_that_is_not_a_beat_is_refused() {
    for beat in [-1.0, f32::NAN, f32::INFINITY] {
        assert!(
            matches!(
                refused(vec![curve(Param::Gain, vec![at(beat, 0.5)])]),
                SynthError::BadAutomationPoint { field, .. } if field == "beat"
            ),
            "{beat} is not a beat"
        );
    }
}

#[test]
fn a_value_that_is_not_a_number_is_refused() {
    assert!(matches!(
        refused(vec![curve(Param::Gain, vec![at(0.0, f32::NAN)])]),
        SynthError::BadAutomationPoint { field, .. } if field == "value"
    ));
}

#[test]
fn a_cutoff_curve_through_zero_hertz_is_refused() {
    let mut song = voiced(filtered(400.0), 0.5);
    song.automation = vec![curve(Param::Cutoff, vec![at(0.0, 0.0), at(8.0, 900.0)])];
    assert!(matches!(
        song.validate().expect_err("0 Hz leaves nothing to pass"),
        SynthError::BadAutomationCutoff { cutoff, .. } if cutoff == 0.0
    ));
}

/// The one refusal that needs the instrument rather than the document, so it
/// arrives when the patch is resolved rather than when the song is read.
#[test]
fn a_cutoff_curve_on_an_instrument_with_no_filter_is_refused() {
    let mut song = held(0.5);
    song.automation = vec![curve(Param::Cutoff, vec![at(0.0, 900.0)])];
    song.validate()
        .expect("the document alone cannot know what the patch has");
    assert!(matches!(
        render_song(&song, &InlineOnly).expect_err("there is no filter to move"),
        SynthError::AutomationWithoutFilter { ref track } if track == "pad"
    ));
}

/// The point of a closed list: a misspelled parameter is refused by the reader,
/// against the words that work, rather than parsed into a curve on nothing.
#[test]
fn a_parameter_that_is_not_one_of_the_words_is_refused_by_the_reader() {
    let written = r#"{ "track": "pad", "param": "cutof", "points": [] }"#;
    let refusal = serde_json::from_str::<Automation>(written)
        .expect_err("`cutof` is not a parameter")
        .to_string();
    assert!(
        refusal.contains("cutoff"),
        "it says what would work: {refusal}"
    );
}

/// And an unknown field on a point, for the same reason a note's entry denies
/// them: `{ "beat": 0, "vaule": 1 }` is a point that moves nothing.
#[test]
fn a_misspelled_field_on_a_point_is_refused() {
    let written = r#"{ "beat": 0.0, "vaule": 1.0 }"#;
    serde_json::from_str::<Point>(written).expect_err("`vaule` is not a field");
}
