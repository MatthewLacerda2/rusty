//! A step entry as written down: told apart by its fields, round-tripped, and its `start` kept
//! only when set.

use rusty_zimmer::song::{PatternEntry, Steps};
use rusty_zimmer::Song;

use super::{playing, stepped, steps};

#[test]
fn a_step_entry_is_told_apart_by_its_fields_and_survives_a_round_trip() {
    let written = stepped();
    let reloaded = Song::from_json(&written.to_json().expect("serialise")).expect("deserialise");
    assert_eq!(reloaded, written);
    assert!(
        matches!(reloaded.patterns["verse"].notes[0], PatternEntry::Steps(_)),
        "a step entry came back as something else"
    );

    for confused in [
        r#"{ "track": "b", "chord": "Dm7", "steps": "x-xX", "div": 0.5, "start": 0, "dur": 1 }"#,
        // A misspelled field, which would otherwise be a velocity silently at 1.
        r#"{ "track": "b", "steps": "x-xX", "div": 0.5, "vell": 0.4 }"#,
        r#"{ "track": "b", "steps": "x-xX" }"#,
    ] {
        assert!(
            serde_json::from_str::<PatternEntry>(confused).is_err(),
            "`{confused}` should not have parsed"
        );
    }
}

#[test]
fn a_step_entry_writes_down_its_start_only_when_it_has_one() {
    let json = playing(vec![steps("x-xX").into()])
        .to_json()
        .expect("serialise");
    assert!(
        !json.contains("\"start\""),
        "a string starting at the top of its pattern wrote a `start` it did not need:\n{json}"
    );

    let late = playing(vec![Steps {
        start: 1.0,
        ..steps("xX")
    }
    .into()]);
    let reloaded = Song::from_json(&late.to_json().expect("serialise")).expect("deserialise");
    assert_eq!(reloaded, late, "a `start` that was not the top was lost");
}
