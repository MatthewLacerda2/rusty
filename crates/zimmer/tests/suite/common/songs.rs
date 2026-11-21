//! Song fixtures: the smallest thing that is recognisably a piece of music.

use std::collections::BTreeMap;

use scorsese_zimmer::patch::{Adsr, NoiseColor, Patch, Source};
use scorsese_zimmer::song::{Note, PatchRef, Pattern, PatternEntry, Pitch, Song, Track};

pub(crate) fn blip() -> Patch {
    Patch {
        layers: Vec::new(),
        source: Source::Noise {
            color: NoiseColor::White,
        },
        amp: Adsr {
            a: 0.001,
            d: 0.05,
            s: 0.0,
            r: 0.05,
            curve: 0.0,
        },
        filter: None,
        pitch_env: None,
        lfo: None,
        lfo2: None,
        fx: vec![],
    }
}

pub(crate) fn note(track: &str, name: &str, start: f32, dur: f32) -> Note {
    Note {
        track: track.to_owned(),
        note: Pitch::Name(name.to_owned()),
        start,
        dur,
        vel: 1.0,
        articulation: None,
    }
}

pub(crate) fn played(notes: Vec<Note>) -> Vec<PatternEntry> {
    notes.into_iter().map(PatternEntry::from).collect()
}

pub(crate) fn voice(pattern: &mut Pattern, index: usize) -> &mut Note {
    let PatternEntry::Note(note) = &mut pattern.notes[index] else {
        panic!("entry {index} is a chord, and this fixture's notes are plain")
    };
    note
}

pub(crate) fn song() -> Song {
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "verse".to_owned(),
        Pattern {
            beats: 2.0,
            notes: played(vec![
                note("bass", "E2", 0.0, 0.5),
                note("bass", "B2", 1.0, 0.5),
            ]),
        },
    );
    Song {
        bpm: 120.0,
        tempo: vec![],
        seed: 7,
        key: None,
        tracks: vec![Track {
            name: "bass".to_owned(),
            patch: PatchRef::Inline(Box::new(blip())),
            gain: 0.8,
            pan: 0.0,
            fx: vec![],
            send: 1.0,
            push: 0.0,
            swing: None,
            humanize: None,
        }],
        patterns,
        arrangement: vec!["verse".into(), "verse".into()],
        swing: 0.0,
        humanize: None,
        fx: vec![],
        automation: vec![],
        fit: None,
        anchors: Vec::new(),
        fade: None,
        tail: None,
    }
}

pub(crate) fn verse(song: &mut Song) -> &mut Pattern {
    song.patterns
        .get_mut("verse")
        .expect("the fixture defines `verse`")
}
