//! Every song document type refuses a key it does not know, and says which —
//! including inside the forms a song writes more than one way.
//!
//! The second half is the harder one. A track's patch, an arrangement entry
//! and a pattern entry each accept several shapes, and serde's untagged reader
//! answers a typo in any of them with *"data did not match any variant"*,
//! losing the key. These tests hold that the key survives (#592).

use scorsese_zimmer::Song;
use scorsese_zimmer::song::{ArrangementEntry, Pattern, PatternEntry, Track};
use serde::de::DeserializeOwned;

/// Asserts `json` does not parse as `T`, and that the refusal names `key`.
fn refused<T: DeserializeOwned + std::fmt::Debug>(json: &str, key: &str) {
    let refusal = serde_json::from_str::<T>(json)
        .expect_err("a misspelled key is refused")
        .to_string();
    assert!(
        refusal.contains(&format!("`{key}`")),
        "the refusal names `{key}`: {refusal}"
    );
}

/// A song with one track playing `patch`, one empty pattern, and `extra`
/// spliced in at the top level.
fn song(patch: &str, extra: &str) -> String {
    format!(
        r#"{{ "bpm": 120, "tracks": [ {{ "name": "a", "patch": {patch} }} ],
             "patterns": {{ "p": {{ "beats": 4, "notes": [] }} }},
             "arrangement": [ "p" ] {extra} }}"#
    )
}

#[test]
fn a_song_refuses_an_unknown_top_level_key() {
    Song::from_json(&song(r#""lead""#, "")).expect("the well-spelled song parses");
    refused::<Song>(&song(r#""lead""#, r#", "swng": 0.3"#), "swng");
}

#[test]
fn a_track_refuses_a_misspelled_gain() {
    refused::<Track>(r#"{ "name": "a", "patch": "lead", "gian": 0.5 }"#, "gian");
}

#[test]
fn a_pattern_refuses_a_misspelled_notes() {
    refused::<Pattern>(r#"{ "beats": 4, "notes": [], "note": [] }"#, "note");
}

/// Inside an inline patch, three levels down, through the untagged
/// name-or-patch form.
#[test]
fn an_inline_patch_names_its_misspelled_key() {
    let patch = r#"{ "source": { "kind": "noise" },
                     "amp": { "a": 0, "d": 0, "s": 1, "r": 0, "atack": 1 } }"#;
    refused::<Song>(&song(patch, ""), "atack");
}

#[test]
fn a_play_in_the_arrangement_names_its_misspelled_key() {
    serde_json::from_str::<ArrangementEntry>(r#""chorus""#).expect("a bare name still parses");
    refused::<ArrangementEntry>(r#"{ "pattern": "chorus", "transpse": 3 }"#, "transpse");
}

/// Each of the four forms a pattern entry is written in, misspelled.
#[test]
fn every_pattern_entry_form_names_its_misspelled_key() {
    for (entry, key) in [
        (
            r#"{ "track": "a", "note": "C4", "start": 0, "dur": 1, "velo": 1 }"#,
            "velo",
        ),
        (
            r#"{ "track": "a", "degree": 1, "start": 0, "dur": 1, "octv": 1 }"#,
            "octv",
        ),
        (
            r#"{ "track": "a", "chord": "Dm7", "start": 0, "dur": 1, "apr": "up" }"#,
            "apr",
        ),
        (
            r#"{ "track": "a", "steps": "x-x-", "div": 0.5, "vell": 0.4 }"#,
            "vell",
        ),
    ] {
        refused::<PatternEntry>(entry, key);
    }
}

/// Two forms in one entry are still refused rather than resolved, and the
/// refusal says which key did not belong.
#[test]
fn an_entry_naming_two_forms_is_refused_by_name() {
    let both = r#"{ "track": "a", "chord": "Dm7", "degree": 1, "start": 0, "dur": 1 }"#;
    refused::<PatternEntry>(both, "degree");
}

/// A step string may carry the pitch it plays, so `note` beside `steps` is a
/// field of the steps and not a second form.
#[test]
fn a_step_string_with_a_pitch_is_still_a_step_string() {
    let written = r#"{ "track": "a", "steps": "x-x-", "div": 0.5, "note": "C2" }"#;
    let entry = serde_json::from_str::<PatternEntry>(written).expect("parses");
    assert!(matches!(entry, PatternEntry::Steps(_)), "got {entry:?}");
}

#[test]
fn an_entry_with_no_form_and_a_duplicated_key_are_refused() {
    let none = r#"{ "track": "a", "start": 0, "dur": 1 }"#;
    let refusal = serde_json::from_str::<PatternEntry>(none).expect_err("plays nothing");
    assert!(refusal.to_string().contains("`steps`"), "{refusal}");

    let twice = r#"{ "track": "a", "note": "C4", "start": 0, "dur": 1, "dur": 2 }"#;
    refused::<PatternEntry>(twice, "dur");
}
