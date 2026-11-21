//! Sections pinned to times (#1009): each anchored downbeat lands exactly on
//! its time, the tempo between anchors is what moves, and what the clock
//! cannot honour is refused by name.

mod landing;
mod refusals;

use scorsese_zimmer::Song;
use scorsese_zimmer::song::Anchor;

/// Four sections of four beats at 120 bpm — two seconds each, so a written
/// boundary is a round number and every anchor is a deliberate distance off
/// one.
pub(crate) fn four_sections(anchors: Vec<Anchor>) -> Song {
    let mut song = Song::from_json(
        r#"{
      "bpm": 120, "seed": 1,
      "tracks": [{ "name": "t", "patch": { "source": { "kind": "noise" },
          "amp": { "a": 0.001, "d": 0.05, "s": 0.0, "r": 0.02 } } }],
      "patterns": { "a": { "beats": 4, "notes": [
          { "track": "t", "note": "C3", "start": 0, "dur": 0.5 } ] } },
      "arrangement": ["a", "a", "a", "a"],
      "tail": "exact"
    }"#,
    )
    .expect("the fixture parses");
    song.anchors = anchors;
    song
}

/// Where each section ends, in seconds.
pub(crate) fn ends(song: &Song) -> Vec<f64> {
    song.sections().iter().map(|cut| cut.end_seconds).collect()
}
