//! Beats, bars and the tempo map.

use scorsese_zimmer::midi::import;
use scorsese_zimmer::song::TempoChange;

use crate::midi::file::{Track, smf};
use crate::midi::parts::notes;

#[test]
fn a_file_without_a_tempo_is_at_one_hundred_and_twenty() {
    let mut part = Track::default();
    part.note(0, 480, 0, 60);
    let song = import(&smf(0, 480, &mut [part])).expect("imports").song;
    assert_eq!(song.bpm, 120.0);
    assert!(song.tempo.is_empty());
}

#[test]
fn later_tempo_events_are_jumps_and_a_restatement_is_not_one() {
    let mut conductor = Track::default();
    conductor.tempo(0, 90).tempo(1_920, 120).tempo(3_840, 120);
    let mut part = Track::default();
    part.note(0, 4_800, 0, 60);
    let song = import(&smf(1, 480, &mut [conductor, part]))
        .expect("imports")
        .song;
    assert_eq!(song.bpm, 90.0);
    assert_eq!(song.tempo, [TempoChange::jump(4.0, 120.0)]);
}

#[test]
fn patterns_are_eight_bars_of_the_files_own_meter() {
    // 3/4 throughout, and a note half a beat into bar 9.
    let mut part = Track::default();
    part.meter(0, 3, 2)
        .note(0, 480, 0, 60)
        .note(24 * 480 + 240, 480, 0, 62);
    let song = import(&smf(0, 480, &mut [part])).expect("imports").song;

    let order: Vec<String> = song.arrangement.iter().map(|entry| entry.label()).collect();
    assert_eq!(order, ["bars-1-8", "bar-9"]);
    assert_eq!(song.patterns["bars-1-8"].beats, 24.0);
    assert_eq!(song.patterns["bar-9"].beats, 3.0);
    assert_eq!(
        song.patterns["bar-9"].notes[0].start(),
        0.5,
        "timed from its pattern"
    );
    assert_eq!(song.arrangement_beats(), 27.0);
}

#[test]
fn a_note_across_a_pattern_boundary_keeps_its_length_and_its_onset() {
    let mut part = Track::default();
    // Starts in bar 8, rings two bars into the next pattern.
    part.note(7 * 1_920, 3 * 1_920, 0, 55);
    let song = import(&smf(0, 480, &mut [part])).expect("imports").song;
    let held = &notes(&song, "track-1")[0];
    assert_eq!((held.start, held.dur), (28.0, 12.0));
    assert!(song.patterns["bars-1-8"].notes.len() == 1);
    assert!(
        song.patterns["bars-9-10"].notes.is_empty(),
        "silence still has a slot"
    );
}
