//! Tests for the song document and its mixer (#358).

use std::collections::BTreeMap;

use super::render::{bake_song, render_song};
use super::{Note, PatchRef, Pattern, Pitch, Song, Track};
use crate::soundgen::core::RATE;
use crate::soundgen::patch::{Adsr, Patch, Source};

/// A short plucked patch — cheap to render and stochastic (Karplus is noise-excited),
/// so it exercises the seeding path rather than a purely deterministic oscillator.
fn pluck() -> Patch {
    Patch {
        source: Source::Noise,
        amp: Adsr {
            a: 0.001,
            d: 0.05,
            s: 0.0,
            r: 0.05,
        },
        filter: None,
        lfo: None,
        fx: vec![],
    }
}

fn note(track: &str, name: &str, start: f32, dur: f32) -> Note {
    Note {
        track: track.to_string(),
        note: Pitch::Name(name.to_string()),
        start,
        dur,
        vel: 1.0,
    }
}

/// Two beats, two notes, one track, arranged twice — the smallest thing that is
/// recognisably a song rather than a note.
fn song() -> Song {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "verse".to_string(),
        Pattern {
            beats: 2.0,
            notes: vec![note("bass", "E2", 0.0, 0.5), note("bass", "B2", 1.0, 0.5)],
        },
    );
    Song {
        bpm: 120.0,
        seed: 7,
        tracks: vec![Track {
            name: "bass".to_string(),
            patch: PatchRef::Inline(Box::new(pluck())),
            gain: 0.8,
        }],
        patterns,
        arrangement: vec!["verse".to_string(), "verse".to_string()],
    }
}

#[test]
fn the_document_round_trips_through_json() {
    let original = song();
    let parsed = Song::from_json(&original.to_json().unwrap()).unwrap();
    assert_eq!(
        parsed, original,
        "a song must survive a save/load round trip"
    );
}

#[test]
fn a_pitch_may_be_a_name_or_a_midi_number() {
    assert_eq!(Pitch::Name("C4".to_string()).to_midi().unwrap(), 60.0);
    assert_eq!(Pitch::Midi(61.5).to_midi().unwrap(), 61.5);
    assert!(Pitch::Name("H4".to_string()).to_midi().is_err());
    // Untagged serde must route a JSON string to the name arm and a number to MIDI.
    let from_name: Pitch = serde_json::from_str(r#""C#4""#).unwrap();
    let from_midi: Pitch = serde_json::from_str("61").unwrap();
    assert_eq!(from_name.to_midi().unwrap(), 61.0);
    assert_eq!(from_midi.to_midi().unwrap(), 61.0);
}

/// Length is `max(arrangement, last tail)` — both halves matter, and for different
/// reasons: the arrangement floor keeps a song that ends on a rest loopable, and the
/// tail overhang keeps a final chord from being chopped mid-ring.
#[test]
fn the_arrangement_is_a_floor_and_the_tail_may_exceed_it() {
    let ends_on_a_rest = song();
    assert_eq!(
        ends_on_a_rest.arrangement_beats(),
        4.0,
        "two 2-beat patterns"
    );
    let arrangement_end =
        (ends_on_a_rest.arrangement_beats() * ends_on_a_rest.beat_seconds() * RATE).round()
            as usize;

    // The last note is a short pluck on beat 3 of 4, so its audio dies well before the
    // arrangement does. The rest at the end must survive into the file anyway.
    let samples = render_song(&ends_on_a_rest).unwrap();
    assert_eq!(
        samples.len(),
        arrangement_end,
        "a song ending on a rest must still be a full arrangement long"
    );

    // Now hold the last note past the final beat: the buffer has to grow for it.
    let mut rings_out = song();
    rings_out.patterns.get_mut("verse").unwrap().notes[1].dur = 4.0;
    assert!(
        render_song(&rings_out).unwrap().len() > arrangement_end,
        "a note held past the final beat must ring out rather than be cut"
    );
}

#[test]
fn the_mix_never_clips() {
    // Many loud notes stacked on the same beat is exactly what overshoots full scale.
    let mut piled = song();
    let verse = piled.patterns.get_mut("verse").unwrap();
    verse.notes = (0..12).map(|_| note("bass", "E2", 0.0, 1.0)).collect();
    piled.tracks[0].gain = 1.0;
    let samples = render_song(&piled).unwrap();
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        peak <= 1.0,
        "the master limiter is not optional (peak {peak})"
    );
}

#[test]
fn the_same_song_and_seed_bake_identical_bytes() {
    let dir = std::env::temp_dir();
    let (a, b) = (
        dir.join("rusty_song_det_a.wav"),
        dir.join("rusty_song_det_b.wav"),
    );
    let seeded = song();
    for path in [&a, &b] {
        bake_song(&seeded, path.to_str().unwrap()).unwrap();
    }
    assert_eq!(
        std::fs::read(&a).unwrap(),
        std::fs::read(&b).unwrap(),
        "same song + seed must bake a byte-identical WAV"
    );

    // ...and a different seed must actually re-roll the stochastic sources.
    let reseeded = Song {
        seed: 8,
        ..seeded.clone()
    };
    assert_ne!(
        render_song(&reseeded).unwrap(),
        render_song(&seeded).unwrap(),
        "changing the song seed must change the render"
    );
    std::fs::remove_file(a).ok();
    std::fs::remove_file(b).ok();
}

#[test]
fn validation_names_what_is_wrong_before_rendering() {
    let bad_tempo = Song { bpm: 0.0, ..song() };
    assert!(bad_tempo.validate().unwrap_err().contains("bpm"));

    let mut unknown_pattern = song();
    unknown_pattern.arrangement = vec!["chorus".to_string()];
    assert!(unknown_pattern.validate().unwrap_err().contains("chorus"));

    let mut unknown_track = song();
    unknown_track.patterns.get_mut("verse").unwrap().notes[0].track = "lead".to_string();
    assert!(unknown_track.validate().unwrap_err().contains("lead"));

    let mut bad_duration = song();
    bad_duration.patterns.get_mut("verse").unwrap().notes[0].dur = 0.0;
    assert!(bad_duration.validate().unwrap_err().contains("dur"));
}

#[test]
fn a_missing_patch_file_is_reported_with_its_track() {
    let mut missing = song();
    missing.tracks[0].patch = PatchRef::Path("no/such/patch.json".to_string());
    let err = render_song(&missing).expect_err("the patch cannot be read");
    assert!(
        err.contains("bass") && err.contains("no/such/patch.json"),
        "got {err}"
    );
}
