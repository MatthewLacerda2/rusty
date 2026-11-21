//! `vel_scale` and `tracks`: how hard a section is played, and by whom.

use crate::common::peak;
use crate::common::songs::{note, played, song, verse, voice};
use scorsese_zimmer::SAMPLE_RATE;
use scorsese_zimmer::song::{ArrangementEntry, Play, Track};

use super::setup::{play, render};

#[test]
fn scaling_velocity_equals_writing_the_notes_quieter() {
    let mut scaled = song();
    scaled.arrangement = vec![ArrangementEntry::from(Play {
        vel_scale: Some(0.5),
        ..play("verse")
    })];

    let mut by_hand = song();
    by_hand.arrangement = vec!["verse".into()];
    let pattern = verse(&mut by_hand);
    for index in 0..pattern.notes.len() {
        voice(pattern, index).vel *= 0.5;
    }

    assert_eq!(render(&scaled), render(&by_hand));
}

/// The arrangement dial: drop to one instrument for a section, and everything
/// coming back is a chorus arriving for free.
#[test]
fn a_tracks_filter_silences_exactly_the_tracks_it_does_not_name() {
    // Two instruments, each playing on its own beat, so "was it silenced?" is a
    // question about a region of the buffer rather than about a sum.
    let mut duet = song();
    duet.tracks.push(Track {
        name: "lead".to_owned(),
        ..duet.tracks[0].clone()
    });
    verse(&mut duet).notes = played(vec![
        note("bass", "E2", 0.0, 0.4),
        note("lead", "B4", 1.0, 0.4),
    ]);
    duet.arrangement = vec!["verse".into()];

    let beat = (duet.beat_seconds() * SAMPLE_RATE as f32) as usize;
    for (only, sounds, silent) in [("bass", 0, beat), ("lead", beat, 0)] {
        let mut filtered = duet.clone();
        filtered.arrangement = vec![ArrangementEntry::from(Play {
            tracks: Some(vec![only.to_owned()]),
            ..play("verse")
        })];
        let rendered = render(&filtered);
        assert!(
            peak(&rendered[sounds..sounds + beat]) > 0.0,
            "`{only}` should still play its own beat"
        );
        assert_eq!(
            peak(&rendered[silent..silent + beat]),
            0.0,
            "the beat `{only}` does not play on should be silent"
        );
    }
}

/// A silenced note still consumes its seed ordinal, so muting a track for a
/// section does not re-roll the noise of every note after it — the fixture's
/// patch is noise precisely so this is audible if it breaks.
#[test]
fn silencing_a_track_leaves_the_other_notes_bit_identical() {
    let mut duet = song();
    duet.tracks.push(Track {
        name: "lead".to_owned(),
        ..duet.tracks[0].clone()
    });
    verse(&mut duet).notes = played(vec![
        note("bass", "E2", 0.0, 0.4),
        note("lead", "B4", 1.0, 0.4),
    ]);

    let mut filtered = duet.clone();
    filtered.arrangement = vec![
        ArrangementEntry::from(Play {
            tracks: Some(vec!["bass".to_owned()]),
            ..play("verse")
        }),
        "verse".into(),
    ];
    duet.arrangement = vec!["verse".into(), "verse".into()];

    let (with_filter, without) = (render(&filtered), render(&duet));
    let beat = (duet.beat_seconds() * SAMPLE_RATE as f32) as usize;
    assert_eq!(
        with_filter[2 * beat..],
        without[2 * beat..],
        "the second pass must be untouched by a filter on the first"
    );
}
