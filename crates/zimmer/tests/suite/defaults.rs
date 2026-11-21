//! A field left out of a document stays out when the document is written back
//! — the rule `common/written.rs` explains and enforces, here on the smallest
//! patch and song the parser takes. The worked recipes in `docs/recipes.md`,
//! which reach every stage, are held to it in
//! `crates/providers/tests/suite/documented`, where the recipe wrapper lives.

#[path = "common/written.rs"]
mod written;

use scorsese_zimmer::{Patch, Song};

/// The smallest patch the parser takes: a source and an envelope.
const PATCH: &str = r#"{
    "source": { "kind": "noise" },
    "amp": { "a": 0.001, "d": 0.05, "s": 0.0, "r": 0.02 }
}"#;

/// The smallest song: one track, one pattern, one note, arranged once.
const SONG: &str = r#"{
    "bpm": 120,
    "tracks": [{ "name": "t", "patch": {
        "source": { "kind": "osc_stack", "oscs": [{ "wave": "saw" }] },
        "amp": { "a": 0.001, "d": 0.05, "s": 0.0, "r": 0.02 }
    } }],
    "patterns": { "a": { "beats": 4, "notes": [
        { "track": "t", "note": "C3", "start": 0, "dur": 1.0 }
    ] } },
    "arrangement": ["a"]
}"#;

#[test]
fn the_smallest_patch_writes_back_only_what_it_was_given() {
    let patch = Patch::from_json(PATCH).expect("the smallest patch parses");
    let rewritten = patch.to_json().expect("serialise");
    same_keys("the smallest patch", "patch", PATCH, &rewritten);
}

#[test]
fn the_smallest_song_writes_back_only_what_it_was_given() {
    let song = Song::from_json(SONG).expect("the smallest song parses");
    let rewritten = song.to_json().expect("serialise");
    same_keys("the smallest song", "", SONG, &rewritten);
}

/// The check itself: a field that appears on the way out is named.
#[test]
#[should_panic(expected = "patch.width")]
fn a_field_written_at_its_default_is_named() {
    let grown = r#"{"source": {"kind": "noise"}, "width": 0.0}"#;
    same_keys(
        "a patch",
        "patch",
        r#"{"source": {"kind": "noise"}}"#,
        grown,
    );
}

/// Fails with [`written::drift`]'s words, if it has any.
fn same_keys(what: &str, root: &str, written: &str, rewritten: &str) {
    if let Some(problem) = written::drift(what, root, written, rewritten) {
        panic!("{problem}");
    }
}
