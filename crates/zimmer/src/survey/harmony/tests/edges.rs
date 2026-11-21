//! Where the report's lines are drawn: what counts as one finding, and the
//! exact edge of a passing note.

use super::{RUB, harmony};
use crate::survey::harmony::percent;

#[test]
fn a_song_with_a_rub_is_not_clean() {
    assert!(!harmony(RUB).is_clean());
}

/// Two rubs at two beats are two findings, and so are two accidentals.
#[test]
fn different_beats_of_one_section_are_different_findings() {
    let found = harmony(
        r#"
  "key": "E major",
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "note": "E3", "start": 0, "dur": 4 },
    { "track": "bass", "note": "F2", "start": 0, "dur": 1 },
    { "track": "bass", "note": "F2", "start": 2, "dur": 1 } ] } },
  "arrangement": ["a"]"#,
    );
    assert_eq!(found.longest.len(), 2);
    assert_eq!(found.out_of_key[0].longest.len(), 2);
    // A quarter of a beat apart is apart too.
    let close = harmony(
        r#"
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "note": "E3", "start": 0, "dur": 4 },
    { "track": "bass", "note": "F2", "start": 1, "dur": 0.25 },
    { "track": "bass", "note": "F2", "start": 1.25, "dur": 0.25 } ] } },
  "arrangement": ["a"]"#,
    );
    assert_eq!(close.longest.len(), 2);
}

/// The same accidental in every chorus is one row, counted each time.
#[test]
fn a_repeated_accidental_is_listed_once_and_counted_every_time() {
    let found = harmony(&format!(
        r#""key": "E major", {}"#,
        RUB.replace(r#"["chorus"]"#, r#"["chorus", "chorus", "chorus"]"#)
    ));
    let strays = &found.out_of_key[0].longest;
    assert_eq!(strays.len(), 1);
    assert_eq!(strays[0].at.times, 3);
}

/// An eighth of a beat is where a passing note stops: shorter is passing,
/// exactly that long is held.
#[test]
fn an_eighth_of_a_beat_exactly_is_held_and_anything_shorter_passes() {
    let at = |dur: f32| {
        harmony(&format!(
            r#"
  "key": "E major",
  "patterns": {{ "a": {{ "beats": 4, "notes": [
    {{ "track": "pad", "note": "E3", "start": 0, "dur": 4 }},
    {{ "track": "bass", "note": "F2", "start": 1, "dur": {dur} }} ] }} }},
  "arrangement": ["a"]"#
        ))
    };
    let held = at(0.125);
    assert_eq!((held.clashes, held.out_of_key.len()), (1, 1));
    let passing = at(0.1);
    assert_eq!((passing.clashes, passing.out_of_key.len()), (0, 0));
}

#[test]
fn a_share_too_small_to_round_to_one_percent_says_so() {
    assert_eq!(percent(0.004), "<1%");
    assert_eq!(percent(0.01), "1%");
    assert_eq!(percent(0.5), "50%");
}
