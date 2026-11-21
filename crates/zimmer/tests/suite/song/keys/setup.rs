//! What every key test is written against: one instrument, one pattern, and the shorthands for
//! putting a degree or a triad into it.

use crate::common::minimal;
use crate::common::songs::{note, played, song, verse};
use rusty_zimmer::patch::Source;
use rusty_zimmer::song::{
    ArrangementEntry, Degree, DegreeNote, InlineOnly, PatchRef, PatternEntry, Play,
};
use rusty_zimmer::{render_song, Song};

pub(crate) fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

pub(crate) fn playing(key: Option<&str>, entries: Vec<PatternEntry>) -> Song {
    let mut one_pass = song();
    one_pass.arrangement = vec!["verse".into()];
    one_pass.key = key.map(str::to_owned);
    one_pass.tracks[0].patch = PatchRef::Inline(Box::new(minimal(Source::Karplus {
        damping: 0.99,
        brightness: 0.5,
    })));
    verse(&mut one_pass).notes = entries;
    one_pass
}

pub(crate) fn degree(written: Degree, oct: i32, start: f32) -> PatternEntry {
    DegreeNote {
        track: "bass".to_owned(),
        degree: written,
        oct: Some(oct),
        start,
        dur: 0.5,
        vel: 1.0,
        articulation: None,
    }
    .into()
}

pub(crate) fn lifted(song: &mut Song, transpose: Option<f32>, degrees: Option<i32>) {
    song.arrangement = vec![ArrangementEntry::from(Play {
        pattern: "verse".to_owned(),
        transpose,
        transpose_degrees: degrees,
        vel_scale: None,
        tracks: None,
    })];
}

pub(crate) fn triad(names: [&str; 3]) -> Vec<PatternEntry> {
    played(
        names
            .iter()
            .enumerate()
            .map(|(index, name)| note("bass", name, index as f32 * 0.5, 0.5))
            .collect(),
    )
}
