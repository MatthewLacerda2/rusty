//! An imported file, exported and imported again, is the song it was.
//!
//! The strongest thing the two halves can promise each other: every note,
//! every velocity, the tempo map and the key make the trip — and so do the
//! patterns, since the bars fall where they did.

use scorsese_zimmer::midi::{Drum, export, import};

use crate::midi::file::{Track, smf};

/// LilyPond's resolution, which the Mutopia Project's files are written at —
/// and not the exporter's, so the trip has to convert ticks both ways.
const PPQ: u64 = 384;

fn transcription() -> Vec<u8> {
    let mut conductor = Track::default();
    conductor
        .tempo(0, 100)
        .key(0, -3, false)
        .tempo(8 * PPQ, 132);
    let mut piano = Track::default();
    piano.name("Piano");
    // Every velocity MIDI has, an eighth note each: the one number the trip
    // rounds through three decimal places and back.
    for vel in 1..=127_u8 {
        let at = u64::from(vel - 1) * PPQ / 2;
        let key = 48 + vel % 24;
        piano.on(at, 0, key, vel).off(at + PPQ / 2, 0, key);
    }
    // A triplet and a held chord, off the eighth-note grid.
    let later = 64 * PPQ;
    piano
        .on(later, 0, 60, 90)
        .on(later, 0, 64, 80)
        .off(later + PPQ / 3, 0, 64)
        .on(later + PPQ / 3, 0, 67, 70)
        .off(later + 2 * PPQ / 3, 0, 67)
        .off(later + 4 * PPQ, 0, 60);
    let mut kit = Track::default();
    kit.note(0, PPQ / 4, 9, 36)
        .note(PPQ, PPQ / 4, 9, 38)
        .note(3 * PPQ / 2, PPQ / 8, 9, 42);
    smf(1, PPQ as u16, &mut [conductor, piano, kit])
}

#[test]
fn an_imported_file_comes_back_as_the_same_song() {
    let first = import(&transcription()).expect("imports");
    assert!(first.left_out.is_empty(), "{:?}", first.left_out);
    let drums: Drum = "drums".parse().expect("reads");
    let exported = export(&first.song, &[drums]).expect("exports");
    assert!(exported.left_out.is_empty(), "{:?}", exported.left_out);
    assert_eq!(exported.tracks, 2);
    assert_eq!(exported.notes, 127 + 3 + 3);
    assert_eq!(exported.tempo_events, 2);

    let again = import(&exported.bytes).expect("reimports");
    assert!(again.left_out.is_empty(), "{:?}", again.left_out);
    assert_eq!(again.song, first.song);
}

/// Without naming the drum track, it is exported as a pitched part — which
/// is a legitimate file, and the numbers survive as the pitches they are.
#[test]
fn a_drum_part_not_named_as_one_keeps_its_keys_as_pitches() {
    let first = import(&transcription()).expect("imports");
    let again = import(&export(&first.song, &[]).expect("exports").bytes).expect("reimports");
    assert_eq!(again.song.tracks.len(), 2);
    assert_eq!(again.song.tracks[1].name, "drums", "named for what it was");
    assert_ne!(
        again.song.tracks[1].patch, first.song.tracks[1].patch,
        "but played as a pitched part"
    );
}
