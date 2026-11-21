//! The fixture every arpeggio test is written against.

use crate::common::songs::{note, played, song, verse};
use scorsese_zimmer::song::{Arp, Chord, InlineOnly, Note, PatternEntry, Voicing};
use scorsese_zimmer::{Song, render_song};

/// The one-track fixture over four beats, playing `entries` once and quietly
/// enough that the master limiter never acts.
pub(crate) fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut once = song();
    once.arrangement = vec!["verse".into()];
    once.tracks[0].gain = 0.15;
    let verse = verse(&mut once);
    verse.beats = 4.0;
    verse.notes = entries;
    once
}

/// A four-beat chord on the fixture's track, arpeggiated as the fields say.
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

/// The same figure written out one note at a time, on the same grid — the
/// pitches in the order they sound, which is how the page prints them too.
fn by_hand(pitches: &str, div: f32, gate: f32) -> Vec<Note> {
    pitches
        .split_whitespace()
        .enumerate()
        .map(|(step, name)| note("bass", name, step as f32 * div, gate))
        .collect()
}

/// Rendered samples, which is where the two spellings have to agree.
fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// That the shorthand is the long form. The fixture's instrument is
/// *stochastic* on purpose: matching here means every note of the figure took
/// the ordinal it would have taken written out, noise seed and all.
pub(crate) fn same(chord: Chord, pitches: &str, div: f32, gate: f32) {
    assert_eq!(
        render(&playing(vec![chord.into()])),
        render(&playing(played(by_hand(pitches, div, gate)))),
        "the figure did not play the notes it stands for"
    );
}

/// The same song under a player who neither clocks nor lands square, so what a
/// comparison proves is that expansion happened before the performance.
pub(crate) fn scattered(song: Song, feel: scorsese_zimmer::song::Humanize) -> Vec<f32> {
    render(&Song {
        humanize: Some(feel),
        swing: 0.3,
        ..song
    })
}

/// The figure written by hand, as pattern entries.
pub(crate) fn spelled(pitches: &str, div: f32, gate: f32) -> Vec<PatternEntry> {
    played(by_hand(pitches, div, gate))
}
