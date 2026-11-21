//! A chord voiced other than in root position, held to the notes it stands
//! for: `inv` and `voicing: "near"` must render exactly as the pitches they
//! choose, written out.

use crate::common::songs::{note, played, song, verse};
use scorsese_zimmer::song::{Chord, InlineOnly, Leading, Note, PatternEntry, Voicing};
use scorsese_zimmer::{Song, render_song};

/// The one-track fixture over four beats, playing `entries` once and quietly
/// enough that the master limiter never acts.
fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut once = song();
    once.arrangement = vec!["verse".into()];
    once.tracks[0].gain = 0.15;
    let verse = verse(&mut once);
    verse.beats = 4.0;
    verse.notes = entries;
    once
}

/// A two-beat named chord at `start` on the fixture's track, voiced as asked.
fn chord(name: &str, start: f32, inv: Option<u8>, voicing: Option<Leading>) -> PatternEntry {
    Chord {
        track: "bass".to_owned(),
        chord: Voicing::Name(name.to_owned()),
        oct: Some(3),
        start,
        dur: 2.0,
        vel: 1.0,
        articulation: None,
        arp: None,
        div: None,
        gate: None,
        inv,
        voicing,
    }
    .into()
}

/// The same chords written out a voice at a time, one group per two beats.
fn by_hand(groups: &[&str]) -> Song {
    let notes: Vec<Note> = groups
        .iter()
        .enumerate()
        .flat_map(|(at, names)| {
            names
                .split_whitespace()
                .map(move |name| note("bass", name, at as f32 * 2.0, 2.0))
        })
        .collect();
    playing(played(notes))
}

/// Rendered samples, which is where the two spellings have to agree.
fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// The issue's progression: root position jumps every voice, `near` keeps the
/// shared notes — and either way the chord is the notes it names.
#[test]
fn near_plays_the_voicing_it_chose() {
    let near = Some(Leading::Near);
    let led = playing(vec![
        chord("Dmaj7", 0.0, None, near),
        chord("Bm7", 2.0, None, near),
    ]);
    assert_eq!(
        render(&led),
        render(&by_hand(&["D3 F#3 A3 C#4", "D3 F#3 A3 B3"]))
    );
}

/// An inversion is the chord turned, and a pattern placed twice leads from
/// nothing each time: its first `near` chord is root position on every pass.
#[test]
fn inversions_and_the_pattern_edge() {
    let turned = playing(vec![chord("C", 0.0, Some(1), None)]);
    assert_eq!(render(&turned), render(&by_hand(&["E3 G3 C4"])));

    let mut twice = playing(vec![chord("Bm7", 0.0, None, Some(Leading::Near))]);
    twice.arrangement = vec!["verse".into(), "verse".into()];
    let mut root = by_hand(&["B3 D4 F#4 A4"]);
    root.arrangement = twice.arrangement.clone();
    assert_eq!(render(&twice), render(&root));
}

/// The words round-trip, and a chord that asks for neither writes neither — a
/// bake is addressed by its recipe's bytes.
#[test]
fn the_fields_round_trip_and_are_silent_at_their_default() {
    let asked = playing(vec![
        chord("C", 0.0, Some(2), None),
        chord("F", 2.0, None, Some(Leading::Near)),
    ]);
    let json = asked.to_json().expect("serialises");
    assert!(
        json.contains("\"inv\": 2") && json.contains("\"voicing\": \"near\""),
        "{json}"
    );
    assert_eq!(Song::from_json(&json).expect("reads back"), asked);

    let plain = playing(vec![chord("C", 0.0, None, None)])
        .to_json()
        .expect("serialises");
    assert!(
        !plain.contains("\"inv\"") && !plain.contains("\"voicing\""),
        "{plain}"
    );
}
