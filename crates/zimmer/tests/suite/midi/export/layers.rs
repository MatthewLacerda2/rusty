//! A layered entry exports as what it plays: every layer from the slot's first
//! beat, each with its own transforms, and the next entry after the longest.

use scorsese_zimmer::midi::export;

use super::read::{read, strikes};
use super::song;

#[test]
fn every_layer_starts_on_the_slot_and_the_next_entry_waits_for_the_longest() {
    let json = r#""bpm": 120,
        "arrangement": [{ "layers": ["groove", { "pattern": "solo", "transpose": 12 }] }, "groove"],
        "patterns": {
          "groove": { "beats": 2, "notes": [
            { "track": "bass", "note": "C2", "start": 0, "dur": 1 } ] },
          "solo": { "beats": 4, "notes": [
            { "track": "lead", "note": "C4", "start": 1, "dur": 1 } ] } }"#;
    let file = read(
        &export(&song(&["bass", "lead"], json), &[])
            .expect("exports")
            .bytes,
    );
    assert_eq!(
        strikes(&file.tracks[1]),
        [(0, 0, 36, 127), (7_680, 0, 36, 127)],
        "the second groove waits out the four-beat solo"
    );
    assert_eq!(strikes(&file.tracks[2]), [(1_920, 1, 72, 127)]);
}
