use super::*;
use crate::song::InlineOnly;

mod edges;
mod keyed;

/// A song from its `tracks`, `patterns`, `arrangement` and anything else at
/// the top level (a `key`, a `fit`), as JSON fragments.
fn song(rest: &str) -> Song {
    let json = format!(
        r#"{{ "bpm": 120,
          "tracks": [
            {{ "name": "bass", "patch": {{ "source": {{ "kind": "osc_stack", "oscs": [{{ "wave": "saw" }}] }},
                "amp": {{ "a": 0.01, "d": 0.1, "s": 0.8, "r": 0.1 }} }} }},
            {{ "name": "pad", "patch": {{ "source": {{ "kind": "osc_stack", "oscs": [{{ "wave": "sine" }}] }},
                "amp": {{ "a": 0.2, "d": 0.1, "s": 0.8, "r": 0.5 }} }} }},
            {{ "name": "hat", "patch": {{ "source": {{ "kind": "noise" }},
                "amp": {{ "a": 0.001, "d": 0.05, "s": 0.0, "r": 0.05 }} }} }},
            {{ "name": "tom", "patch": {{ "source": {{ "kind": "osc_stack", "oscs": [{{ "wave": "sine" }}] }},
                "amp": {{ "a": 0.001, "d": 0.2, "s": 0.0, "r": 0.1 }} }} }}
          ],
          {rest} }}"#
    );
    Song::from_json(&json).expect("the fixture parses")
}

pub(super) fn harmony(rest: &str) -> Harmony {
    Harmony::of(&song(rest), &InlineOnly)
}

/// The fault the report is for: a bass on F under a pad holding an E-major
/// chord, for two beats in the middle of the bar.
pub(super) const RUB: &str = r#"
  "patterns": { "chorus": { "beats": 4, "notes": [
    { "track": "pad", "chord": "E", "oct": 3, "start": 0, "dur": 4 },
    { "track": "bass", "note": "E2", "start": 0, "dur": 2 },
    { "track": "bass", "note": "F2", "start": 2, "dur": 2 } ] } },
  "arrangement": ["chorus"]"#;

#[test]
fn a_semitone_held_under_a_chord_is_found_where_and_for_as_long_as_it_sounds() {
    let found = harmony(RUB);
    assert_eq!(found.clashes, 1);
    assert_eq!(found.clash_beats, 2.0);
    let clash = &found.longest[0];
    assert_eq!(
        clash.to_string(),
        "2 beats: bass F2 against pad E3, chorus beat 2"
    );
    assert_eq!(
        found.headline().as_deref(),
        Some("harmony: 1 clash, longest 2 beats: bass F2 against pad E3, chorus beat 2")
    );
}

/// A chromatic sixteenth passing over the chord is music, not a mistake.
#[test]
fn a_passing_note_shorter_than_an_eighth_of_a_beat_is_not_a_clash() {
    let found = harmony(
        r#"
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "note": "E3", "start": 0, "dur": 4 },
    { "track": "bass", "note": "F2", "start": 1, "dur": 0.1 } ] } },
  "arrangement": ["a"]"#,
    );
    assert_eq!(found.clashes, 0);
    assert!(found.headline().is_none(), "a clean song adds no line");
}

/// A chord's own voices are the author's voicing, and a cluster of them is
/// deliberate.
#[test]
fn a_chords_own_notes_never_clash_with_each_other() {
    let found = harmony(
        r#"
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "chord": ["E3", "F3", "B3"], "start": 0, "dur": 4 } ] } },
  "arrangement": ["a"]"#,
    );
    assert_eq!(found.clashes, 0);
}

/// A noise source has no pitch to clash with, whatever note it is told.
#[test]
fn a_noise_track_is_skipped() {
    let found = harmony(
        r#"
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "note": "E3", "start": 0, "dur": 4 },
    { "track": "hat", "note": "F3", "start": 0, "dur": 4 } ] } },
  "arrangement": ["a"]"#,
    );
    assert_eq!(found.clashes, 0);
}

/// A step string with no `note` plays its default and states nothing; a tom
/// fill that writes its notes is read, and named.
#[test]
fn a_track_that_never_states_a_pitch_is_skipped_and_one_that_does_is_read() {
    let unstated = harmony(
        r#"
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "note": "B3", "start": 0, "dur": 4 },
    { "track": "tom", "steps": "X-X-X-X-", "div": 0.5, "dur": 0.5 } ] } },
  "arrangement": ["a"]"#,
    );
    assert_eq!(unstated.clashes, 0, "the default C4 is not a written note");
    let stated = harmony(
        r#"
  "patterns": { "a": { "beats": 4, "notes": [
    { "track": "pad", "note": "B3", "start": 0, "dur": 4 },
    { "track": "tom", "steps": "X-------", "div": 0.5, "dur": 1, "note": "C3" } ] } },
  "arrangement": ["a"]"#,
    );
    assert_eq!(stated.clashes, 1);
    assert_eq!(
        stated.longest[0].to_string(),
        "1 beat: pad B3 against tom C3, a beat 0"
    );
}

/// The arithmetic an author gets wrong: one layer of an entry transposed and
/// the other not.
#[test]
fn a_layered_entry_with_one_layer_transposed_is_found() {
    let found = harmony(
        r#"
  "patterns": {
    "groove": { "beats": 4, "notes": [ { "track": "bass", "note": "E2", "start": 0, "dur": 4 } ] },
    "solo": { "beats": 4, "notes": [ { "track": "pad", "note": "E4", "start": 0, "dur": 4 } ] } },
  "arrangement": ["groove", { "layers": ["groove", { "pattern": "solo", "transpose": 1 }] }]"#,
    );
    assert_eq!(found.clashes, 1);
    assert_eq!(
        found.longest[0].to_string(),
        "4 beats: bass E2 against pad F4, groove + solo beat 0"
    );
}

/// The same rub in every chorus is one finding, counted each time.
#[test]
fn a_repeated_clash_is_listed_once_and_counted_every_time() {
    let found = harmony(&RUB.replace(r#"["chorus"]"#, r#"["chorus", "chorus", "chorus"]"#));
    assert_eq!(found.clashes, 3);
    assert_eq!(found.clash_beats, 6.0);
    assert_eq!(found.longest.len(), 1);
    assert_eq!(found.longest[0].at.times, 3);
    assert!(
        found.longest[0]
            .to_string()
            .ends_with("chorus beat 2, 3 times")
    );
}
