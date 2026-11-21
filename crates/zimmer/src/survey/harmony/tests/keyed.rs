//! The key section, and the one case a loop adds: a note ringing over the
//! top of the next pass.

use super::{RUB, harmony};

/// Under a loop, the last bar's ring runs into the first bar's notes.
#[test]
fn a_note_held_over_a_loop_point_clashes_with_the_top_of_the_piece() {
    let looped = r#"
  "key": "E major", "fit": { "mode": "loop", "seconds": 8 },
  "patterns": {
    "a": { "beats": 4, "notes": [ { "track": "pad", "note": "E3", "start": 0, "dur": 4 } ] },
    "b": { "beats": 4, "notes": [ { "track": "bass", "note": "F2", "start": 3, "dur": 2 } ] } },
  "arrangement": ["a", "b"]"#;
    let found = harmony(looped);
    assert_eq!(found.clashes, 1);
    assert_eq!(found.longest[0].at.section, "a");
    assert_eq!(
        found.longest[0].beats, 1.0,
        "only the beat the copy still rings"
    );
    let bass = &found.out_of_key[0];
    assert_eq!(bass.longest[0].at.times, 1, "the copy is not a second note");
    assert_eq!(bass.longest.len(), 1, "nor a second row");
    let once = harmony(&looped.replace(r#""fit": { "mode": "loop", "seconds": 8 },"#, ""));
    assert_eq!(once.clashes, 0, "played once, nothing follows the last bar");
}

#[test]
fn an_in_key_song_reports_nothing() {
    let found = harmony(
        r#"
  "key": "E major",
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "chord": "E", "oct": 3, "start": 0, "dur": 4 },
    { "track": "bass", "note": "B1", "start": 0, "dur": 4 } ] } },
  "arrangement": ["a"]"#,
    );
    assert!(found.is_clean());
    assert_eq!(found.headline(), None);
    assert_eq!(found.to_string(), "clashes: none\nout of E major: none");
}

#[test]
fn a_note_outside_the_declared_key_is_reported_by_its_share_of_held_time() {
    let found = harmony(&format!(r#""key": "E major", {RUB}"#));
    assert_eq!(found.out_of_key.len(), 1);
    let bass = &found.out_of_key[0];
    assert_eq!(bass.track, "bass");
    assert_eq!(bass.share, 0.5, "F2 for two of the bass's four beats");
    assert_eq!(bass.longest[0].to_string(), "F2 for 2 beats, chorus beat 2");
    assert!(
        found
            .headline()
            .expect("something to say")
            .ends_with("; out of E major: bass 50%")
    );
}

#[test]
fn a_song_with_no_key_has_no_key_section_and_none_is_guessed() {
    let found = harmony(RUB);
    assert_eq!(found.key, None);
    assert!(found.out_of_key.is_empty());
    assert!(!found.to_string().contains("out of"));
}
