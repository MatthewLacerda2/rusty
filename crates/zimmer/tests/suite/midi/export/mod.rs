//! A song written out as MIDI: the bytes, what is played into them, the tempo
//! map, and an imported file making the trip back.

mod bytes;
mod layers;
mod played;
mod read;
mod round_trip;
mod tempo;

use scorsese_zimmer::song::Song;

/// A song from its JSON, with one plain inline patch per track named in
/// `tracks` — the sound is never exported, so every test uses the same one.
pub(crate) fn song(tracks: &[&str], rest: &str) -> Song {
    let patch = r#"{ "source": { "kind": "noise" },
                     "amp": { "a": 0.001, "d": 0.1, "s": 1.0, "r": 0.05 } }"#;
    let tracks: Vec<String> = tracks
        .iter()
        .map(|name| format!(r#"{{ "name": "{name}", "patch": {patch} }}"#))
        .collect();
    let json = format!(r#"{{ "tracks": [{}], {rest} }}"#, tracks.join(","));
    Song::from_json(&json).unwrap_or_else(|error| panic!("{error}\n{json}"))
}
