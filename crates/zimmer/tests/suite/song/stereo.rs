//! Where a part sits across the image, and what that does to the numbers.

use std::collections::BTreeMap;

use crate::common::songs::{note, played};
use crate::common::{channel, peak, saw_patch};
use scorsese_zimmer::song::{InlineOnly, PatchRef, Pattern, Song, Track};
use scorsese_zimmer::{bake_song, render_song};

/// A duet of the same instrument twice, so the only thing that can separate
/// the two rows is where each one was put.
///
/// A saw rather than the noise blip the other song tests use: `noise` is the
/// one source that is already two different signals, and a fixture that
/// arrives wide cannot say what the pan did.
fn duet(left: f32, right: f32) -> Song {
    let track = |name: &str, pan: f32| Track {
        name: name.to_owned(),
        patch: PatchRef::Inline(Box::new(saw_patch())),
        // Well under the ceiling, so the master limiter never acts. Hard over
        // is +3 dB on the side it went to, and a fixture that clipped there
        // would be measuring the limiter rather than the pan law.
        gain: 0.5,
        pan,
        fx: vec![],
        send: 1.0,
        push: 0.0,
        swing: None,
        humanize: None,
    };
    let mut patterns = BTreeMap::new();
    patterns.insert(
        "verse".to_owned(),
        Pattern {
            beats: 2.0,
            notes: played(vec![
                note("near", "E3", 0.0, 1.0),
                note("far", "E3", 0.0, 1.0),
            ]),
        },
    );
    Song {
        bpm: 120.0,
        tempo: vec![],
        seed: 7,
        key: None,
        tracks: vec![track("near", left), track("far", right)],
        patterns,
        arrangement: vec!["verse".into()],
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

/// The two channels of a rendered song.
fn sides(song: &Song) -> (Vec<f32>, Vec<f32>) {
    let mix = render_song(song, &InlineOnly).expect("the song renders");
    (channel(&mix, 0), channel(&mix, 1))
}

/// The whole point of the field: a part put to one side comes out of that
/// side, and the other one does not carry it.
#[test]
fn a_hard_panned_track_arrives_on_the_side_it_was_sent_to() {
    let (left, right) = sides(&duet(-1.0, -1.0));
    assert!(
        peak(&left) > 0.1,
        "nothing reached the left ({})",
        peak(&left)
    );
    assert_eq!(peak(&right), 0.0, "and nothing bled to the right");
}

/// A track that never mentions `pan` renders what it always did, in both
/// channels — the promise a default has to keep, checked on real audio rather
/// than on the gain table alone.
#[test]
fn a_song_that_pans_nothing_is_the_same_signal_twice() {
    let (left, right) = sides(&duet(0.0, 0.0));
    assert!(peak(&left) > 0.1, "the fixture makes a sound");
    assert_eq!(left, right, "centre is unity on both sides, exactly");
}

/// The fixture with only its first part playing.
///
/// One instrument, because the question below is what moving *a part* costs:
/// two identical parts sum coherently in the middle and not at the edges, so a
/// duet would be measuring how they add rather than how they are placed.
fn solo(pan: f32) -> Song {
    let mut song = duet(pan, 0.0);
    song.patterns
        .get_mut("verse")
        .expect("the fixture defines `verse`")
        .notes
        .truncate(1);
    song
}

/// Constant power: moving a part off centre must not change how much energy it
/// carries. A linear pan law loses 3 dB here, which is a pan control that
/// doubles as a fader.
#[test]
fn moving_a_part_off_centre_does_not_change_its_energy() {
    let energy = |song: &Song| {
        let (left, right) = sides(song);
        left.iter().chain(&right).map(|s| s * s).sum::<f32>()
    };
    let centred = energy(&solo(0.0));
    for pan in [-1.0, -0.5, 0.5, 1.0] {
        let moved = energy(&solo(pan));
        assert!(
            (moved / centred - 1.0).abs() < 0.02,
            "pan {pan} carries {moved} against {centred} centred"
        );
    }
}

/// A row measures both channels, so a part that is all on one side reads at
/// the level it is playing at rather than at half of it. Measuring one
/// channel, or a fold-down, would report the hard-panned track several
/// decibels under the centred one and send its author to turn it up.
#[test]
fn a_hard_panned_track_does_not_read_quiet() {
    let level = |song: &Song, index: usize| {
        bake_song(song, &InlineOnly)
            .expect("the fixture song bakes")
            .tracks[index]
            .level
            .loudness
            .mean_dbfs
            .expect("the track played")
    };
    let centred = level(&duet(0.0, 0.0), 0);
    let panned = level(&duet(-1.0, 0.0), 0);
    assert!(
        (panned - centred).abs() < 0.5,
        "the same part reads {panned} dBFS panned and {centred} dBFS centred"
    );
}

/// The bytes of a recipe are its cache key, so a field nobody set must not
/// appear — and one somebody set must.
#[test]
fn a_centred_track_writes_no_pan_and_a_placed_one_does() {
    let plain = duet(0.0, 0.0);
    let json = plain.to_json().expect("serialise");
    let written: serde_json::Value = serde_json::from_str(&json).expect("it is JSON");
    assert_eq!(
        written["tracks"][0].get("pan"),
        None,
        "dead centre was written down:\n{json}"
    );
    assert_eq!(Song::from_json(&json).expect("deserialise"), plain);

    let placed = duet(-0.5, 0.0);
    let json = placed.to_json().expect("serialise");
    let written: serde_json::Value = serde_json::from_str(&json).expect("it is JSON");
    assert_eq!(written["tracks"][0]["pan"], -0.5);
    assert_eq!(Song::from_json(&json).expect("deserialise"), placed);
}

/// Out of range is clamped to hard over rather than refused: there is no
/// position past the edge for a bigger number to mean.
#[test]
fn a_pan_past_the_edge_is_the_edge() {
    assert_eq!(sides(&duet(-3.0, 0.0)), sides(&duet(-1.0, 0.0)));
}
