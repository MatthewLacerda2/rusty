//! How a tempo map is written, and what is refused.

use scorsese_zimmer::SynthError;
use scorsese_zimmer::song::{Song, TempoChange};

use crate::common::songs::song;

fn with(tempo: Vec<TempoChange>) -> Song {
    Song { tempo, ..song() }
}

#[test]
fn a_song_without_a_map_saves_without_one() {
    let json = song().to_json().expect("serialises");
    assert!(!json.contains("tempo"), "{json}");
}

#[test]
fn a_jump_is_saved_without_its_ramp_and_a_ramp_with_it() {
    let json = with(vec![
        TempoChange::jump(2.0, 90.0),
        TempoChange::ramp(3.0, 100.0),
    ])
    .to_json()
    .expect("serialises");
    assert_eq!(json.matches("\"ramp\"").count(), 1, "{json}");
    let back = Song::from_json(&json).expect("parses");
    assert_eq!(back.tempo[1], TempoChange::ramp(3.0, 100.0));
    assert_eq!(
        back,
        Song::from_json(&back.to_json().expect("again")).expect("and back")
    );
}

#[test]
fn a_misspelt_field_on_a_change_is_refused_rather_than_ignored() {
    let json = song().to_json().expect("serialises").replacen(
        "\"bpm\": 120.0",
        "\"bpm\": 120.0, \"tempo\": [{ \"beat\": 2, \"bpm\": 90, \"rmap\": true }]",
        1,
    );
    assert!(Song::from_json(&json).is_err());
}

#[test]
fn a_change_on_the_first_beat_or_at_no_tempo_is_refused() {
    for (beat, bpm) in [(0.0, 90.0), (-1.0, 90.0), (2.0, 0.0), (2.0, f32::NAN)] {
        let refusal = with(vec![TempoChange::jump(beat, bpm)]).validate();
        assert!(
            matches!(refusal, Err(SynthError::BadTempoChange { index: 0, .. })),
            "beat {beat} at {bpm}: {refusal:?}"
        );
    }
}

#[test]
fn changes_that_do_not_ascend_are_refused() {
    let refusal = with(vec![
        TempoChange::jump(3.0, 90.0),
        TempoChange::ramp(3.0, 100.0),
    ])
    .validate();
    assert_eq!(
        refusal,
        Err(SynthError::TempoOutOfOrder {
            index: 1,
            beat: 3.0,
            previous: 3.0,
        })
    );
}
