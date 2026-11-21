//! How a mark composes with everything else that decides a note.

use super::setup::{note, playing, rendered, TRACK};
use crate::common::{brightness, peak, saw_patch};
use rusty_zimmer::song::Articulation::{Accent, Ghost};
use rusty_zimmer::song::{Automation, Easing, Humanize, Param, PatchRef, Play, Point};

#[test]
fn a_mark_multiplies_what_the_section_and_the_page_already_decided() {
    let played = |vel, mark| {
        let mut song = playing(vec![note("E2", 1.0, 1.0, vel, mark).into()]);
        // No filter, so the accent's brightness offset cannot reach the samples and the comparison
        // is about the velocity chain alone.
        song.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
        song.humanize = Some(Humanize {
            velocity: 0.2,
            timing: 0.01,
            timbre: 0.0,
        });
        song.arrangement = vec![Play {
            pattern: "verse".to_owned(),
            vel_scale: Some(0.5),
            transpose: None,
            transpose_degrees: None,
            tracks: None,
        }
        .into()];
        rendered(&song)
    };
    let accented = played(0.5, Some(Accent));
    let written_louder = played(0.65, None);
    assert_eq!(accented.len(), written_louder.len());
    for (mark, plain) in accented.iter().zip(&written_louder) {
        assert!((mark - plain).abs() < 1e-6, "{mark} against {plain}");
    }
}

#[test]
fn a_mark_still_opens_a_filter_a_curve_has_already_moved() {
    let under_a_build = |mark| {
        let mut song = playing(vec![note("E2", 1.0, 1.0, 0.5, mark).into()]);
        let point = |beat, value| Point {
            beat,
            value,
            easing: Easing::Linear,
        };
        song.automation = vec![Automation {
            track: TRACK.to_owned(),
            param: Param::Cutoff,
            points: vec![point(0.0, 500.0), point(4.0, 3000.0)],
        }];
        rendered(&song)
    };
    let (plain, accented) = (under_a_build(None), under_a_build(Some(Accent)));
    assert!(
        brightness(&accented) > brightness(&plain),
        "the mark was lost"
    );
    assert!(peak(&accented) > peak(&plain), "and so was its level");
}

#[test]
fn the_players_touch_is_added_to_a_marks_and_not_taken_off_it() {
    let ghost = |seed, timbre| {
        let mut song = playing(vec![note("E2", 1.0, 1.0, 0.5, Some(Ghost)).into()]);
        song.seed = seed;
        song.humanize = Some(Humanize {
            timing: 0.0,
            velocity: 0.0,
            timbre,
        });
        brightness(&rendered(&song))
    };
    let (touched, marked) = (ghost(4, 0.6), ghost(4, 0.0));
    assert!(
        touched > marked * 1.1,
        "seed 4 draws upward, so the touch opens the ghost: {touched} against {marked}"
    );
    let (touched, marked) = (ghost(5, 0.6), ghost(5, 0.0));
    assert!(
        touched < marked * 0.9,
        "seed 5 draws downward, so it closes it further: {touched} against {marked}"
    );
}
