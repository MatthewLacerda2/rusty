//! How a mark composes with everything else that decides a note.
//!
//! [`marks`](super::marks) beside this asserts what each of the three marks
//! does on its own. What is here is the arithmetic around them: a mark
//! multiplies the velocity the section and the page already settled, it adds to
//! the brightness an automation curve has already moved, and the player's own
//! touch is laid over the mark's rather than against it. Each of those is a
//! place where two decisions meet, which is where the surprising behaviour
//! lives and where a wrong sign hides best.

use super::setup::{TRACK, note, playing, rendered};
use crate::common::{brightness, peak, saw_patch};
use scorsese_zimmer::song::Articulation::{Accent, Ghost};
use scorsese_zimmer::song::{Automation, Easing, Humanize, Param, PatchRef, Play, Point};

/// The order of application, as an equality: a mark multiplies the velocity
/// the section and the page already decided, and the player's own scatter is
/// drawn after all three and around them. So an accented note under a
/// `vel_scale` and a `humanize` is the *same samples* as the note written at
/// the level that product comes to — same draw, same seed, same ordinal.
#[test]
fn a_mark_multiplies_what_the_section_and_the_page_already_decided() {
    let played = |vel, mark| {
        let mut song = playing(vec![note("E2", 1.0, 1.0, vel, mark).into()]);
        // No filter, so the accent's brightness offset cannot reach the
        // samples and the comparison is about the velocity chain alone.
        song.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
        song.humanize = Some(Humanize {
            velocity: 0.2,
            timing: 0.01,
            timbre: 0.0,
        });
        song.arrangement = vec![
            Play {
                pattern: "verse".to_owned(),
                vel_scale: Some(0.5),
                transpose: None,
                transpose_degrees: None,
                tracks: None,
            }
            .into(),
        ];
        rendered(&song)
    };
    let accented = played(0.5, Some(Accent));
    let written_louder = played(0.65, None);
    assert_eq!(accented.len(), written_louder.len());
    for (mark, plain) in accented.iter().zip(&written_louder) {
        assert!((mark - plain).abs() < 1e-6, "{mark} against {plain}");
    }
}

/// A `cutoff` curve and a mark are two terms of one sum: the curve moves the
/// base the filter sits at, and the mark adds to it. So an accent under a
/// build is still brighter than the note beside it — the composition the page
/// claims, and the one a reader would want checked.
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

/// A mark's brightness offset and the player's own are **added**, and which way
/// the player's points is the seed's to say.
///
/// The claim above — that a ghost is duller than a note written at its level —
/// pins the mark's own sign and stops there, and every other test here holds
/// `humanize` at nothing. So the sum itself was asserted only as *a difference*,
/// which a renderer subtracting the player's touch makes just as large: that is
/// the mutation that survived #446 unnoticed, and it is the shape
/// `docs/mutation-testing.md` now names.
///
/// Two seeds is what turns "it moved" into a claim about a sign. Both play the
/// same ghost on the same beat, so the mark's own offset is the same number in
/// both; the draw laid over it is a pure function of `(track, ordinal, seed)`
/// through the crate's own hash, and seed 4 draws this note's touch upward
/// where seed 5 draws it down. A sum that subtracted would swap them.
#[test]
fn the_players_touch_is_added_to_a_marks_and_not_taken_off_it() {
    let ghost = |seed, timbre| {
        let mut song = playing(vec![note("E2", 1.0, 1.0, 0.5, Some(Ghost)).into()]);
        song.seed = seed;
        // Spelled out rather than defaulted: the touch under test is the tone
        // one, and the other two axes staying at nothing is what keeps this a
        // reading of that axis alone.
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
