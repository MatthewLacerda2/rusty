//! What each of the three marks actually does to the samples.
//!
//! Every test here compares two renders of the same note, because that is the
//! only claim worth making: a mark is a difference from the note as written,
//! and a measurement of one render alone could not tell one from the other.

use super::setup::{gate, note, playing, rendered, samples, sounding};
use crate::common::{brightness, peak};
use scorsese_zimmer::song::Articulation;
use scorsese_zimmer::song::Articulation::{Accent, Ghost, Staccato};

/// The two renders a test compares: the note as written, and the same note
/// with a mark over it.
fn pair(name: &str, dur: f32, vel: f32, mark: Articulation) -> (Vec<f32>, Vec<f32>) {
    let of = |mark| rendered(&playing(vec![note(name, 1.0, dur, vel, mark).into()]));
    (of(None), of(Some(mark)))
}

/// An accent is louder **and** brighter. Either alone would be a velocity
/// change wearing the name.
#[test]
fn an_accent_is_struck_harder_and_opens_the_instrument_up() {
    let (plain, accented) = pair("E2", 1.0, 0.5, Accent);
    assert!(peak(&accented) > peak(&plain) * 1.1, "no harder");
    assert!(brightness(&accented) > brightness(&plain), "no brighter");
    assert_eq!(gate(&accented), gate(&plain), "an accent is not shorter");
    assert_eq!(sounding(&accented).0, sounding(&plain).0, "nor displaced");
}

/// The half of an accent that velocity alone cannot stand in for: against a
/// note written at the level an accent would reach, it is still the brighter
/// of the two.
#[test]
fn an_accent_is_more_than_the_same_note_written_louder() {
    let accented = rendered(&playing(vec![
        note("E2", 1.0, 1.0, 0.5, Some(Accent)).into(),
    ]));
    let louder = rendered(&playing(vec![note("E2", 1.0, 1.0, 0.65, None).into()]));
    let level = peak(&accented) / peak(&louder);
    assert!((0.9..1.1).contains(&level), "not the same level: {level}");
    assert!(brightness(&accented) > brightness(&louder) * 1.05);
}

/// Staccato is the gate and only the gate — and it is *half* of it, which is
/// what makes a written `dur` of one beat sound as a plain half-beat one.
#[test]
fn staccato_holds_the_note_for_half_of_what_the_page_says() {
    let (long, short) = pair("E5", 1.0, 0.5, Staccato);
    let half = rendered(&playing(vec![note("E5", 1.0, 0.5, 0.5, None).into()]));
    assert!(
        gate(&short).abs_diff(gate(&half)) <= 2 * super::setup::BLOCK,
        "a staccato beat is not a plain half-beat: {} against {}",
        gate(&short),
        gate(&half)
    );
    assert!(gate(&short) < gate(&long) * 3 / 4, "nothing was shortened");
    assert!(
        (peak(&short) / peak(&long) - 1.0).abs() < 0.05,
        "staccato moved the level"
    );
}

/// The written `dur` is what the document still says, which is the whole
/// reason for a mark rather than a shorter number: the rhythm on the page is
/// the rhythm of the music.
#[test]
fn staccato_leaves_the_written_duration_alone() {
    let song = playing(vec![note("E5", 1.0, 1.0, 0.5, Some(Staccato)).into()]);
    rendered(&song);
    let entry = &song.patterns["verse"].notes[0];
    assert_eq!(entry.dur(), 1.0, "the page was rewritten");
    assert_eq!(entry.start(), 1.0, "and moved");
}

/// A ghost is not a quiet note: it is quiet, short, early — and, against a
/// note written at its own level, duller.
#[test]
fn a_ghost_is_quiet_and_short_and_early_and_dead() {
    let (plain, ghosted) = pair("E5", 1.0, 0.8, Ghost);
    assert!(peak(&ghosted) < peak(&plain) * 0.5, "not quiet");
    assert!(gate(&ghosted) < gate(&plain) * 2 / 3, "not short");
    // The direction first, and as its own assertion. `sounding` returns sample
    // indices, so subtracting one from the other is unsigned arithmetic: a
    // ghost that landed *late* would end this test with an overflow panic
    // rather than a failed claim, which is a catch nobody wrote and a
    // `saturating_sub` would silently remove. Early is half of what the mark
    // means, so it is stated.
    let (on_the_beat, ahead_of_it) = (sounding(&plain).0, sounding(&ghosted).0);
    assert!(
        ahead_of_it < on_the_beat,
        "a ghost sits ahead of the beat, not behind it: {ahead_of_it} against {on_the_beat}"
    );
    let early = on_the_beat as f32 - ahead_of_it as f32;
    let expected = samples(0.012);
    assert!(
        (early - expected).abs() < 2.0 * super::setup::BLOCK as f32,
        "{early} samples early, not {expected}"
    );
    let equally_quiet = rendered(&playing(vec![note("E2", 1.0, 1.0, 0.28, None).into()]));
    let dead = rendered(&playing(vec![
        note("E2", 1.0, 1.0, 0.8, Some(Ghost)).into(),
    ]));
    assert!(
        brightness(&dead) < brightness(&equally_quiet) * 0.95,
        "a ghost at the same level is no duller than a quiet note"
    );
}
