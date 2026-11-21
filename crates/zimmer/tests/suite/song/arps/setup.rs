//! The fixture every arpeggio test is written against.

use crate::common::songs::{note, played, song, verse};
use rusty_zimmer::song::{Arp, Chord, InlineOnly, Note, PatternEntry, Voicing};
use rusty_zimmer::{render_song, Song};

pub(crate) fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut once = song();
    once.arrangement = vec!["verse".into()];
    once.tracks[0].gain = 0.15;
    let verse = verse(&mut once);
    verse.beats = 4.0;
    verse.notes = entries;
    once
}

pub(crate) fn arped(name: &str, arp: Option<Arp>, div: Option<f32>, gate: Option<f32>) -> Chord {
    Chord {
        track: "bass".to_owned(),
        chord: Voicing::Name(name.to_owned()),
        oct: Some(3),
        start: 0.0,
        dur: 4.0,
        vel: 1.0,
        articulation: None,
        arp,
        div,
        gate,
        inv: None,
        voicing: None,
    }
}

fn by_hand(pitches: &str, div: f32, gate: f32) -> Vec<Note> {
    pitches
        .split_whitespace()
        .enumerate()
        .map(|(step, name)| note("bass", name, step as f32 * div, gate))
        .collect()
}

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

pub(crate) fn same(chord: Chord, pitches: &str, div: f32, gate: f32) {
    assert_eq!(
        render(&playing(vec![chord.into()])),
        render(&playing(played(by_hand(pitches, div, gate)))),
        "the figure did not play the notes it stands for"
    );
}

pub(crate) fn scattered(song: Song, feel: rusty_zimmer::song::Humanize) -> Vec<f32> {
    render(&Song {
        humanize: Some(feel),
        swing: 0.3,
        ..song
    })
}

pub(crate) fn spelled(pitches: &str, div: f32, gate: f32) -> Vec<PatternEntry> {
    played(by_hand(pitches, div, gate))
}
