//! What every key test is written against: one instrument, one pattern, and
//! the shorthands for putting a degree or a triad into it.
//!
//! The instrument is a **plucked string** on purpose. It is pitched, so a
//! degree that resolves to the wrong note fails rather than passing silently,
//! and it is stochastic, so byte-equality between two renders also says the
//! entries took the same ordinals — and therefore the same seeds — as each
//! other. A noise patch would prove the second and none of the first.

use crate::common::minimal;
use crate::common::songs::{note, played, song, verse};
use scorsese_zimmer::patch::Source;
use scorsese_zimmer::song::{
    ArrangementEntry, Degree, DegreeNote, InlineOnly, PatchRef, PatternEntry, Play,
};
use scorsese_zimmer::{Song, render_song};

/// Renders a song whose instruments are all inline.
pub(crate) fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// The fixture in `key`, playing `entries` once through.
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

/// One degree on the fixture's `bass` track, half a beat long.
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

/// The fixture's one pattern, played with the transposes given.
pub(crate) fn lifted(song: &mut Song, transpose: Option<f32>, degrees: Option<i32>) {
    song.arrangement = vec![ArrangementEntry::from(Play {
        pattern: "verse".to_owned(),
        transpose,
        transpose_degrees: degrees,
        vel_scale: None,
        tracks: None,
    })];
}

/// The three notes of a D minor triad, by name, at half a beat each.
pub(crate) fn triad(names: [&str; 3]) -> Vec<PatternEntry> {
    played(
        names
            .iter()
            .enumerate()
            .map(|(index, name)| note("bass", name, index as f32 * 0.5, 0.5))
            .collect(),
    )
}
