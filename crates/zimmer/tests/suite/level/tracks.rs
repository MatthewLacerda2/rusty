//! One row per track: which layer of the mix is taking up the room.

use std::collections::BTreeMap;

use crate::common::songs::{note, played};
use crate::common::{adsr, osc};
use scorsese_zimmer::level::Layer;
use scorsese_zimmer::patch::{Patch, Source, Wave};
use scorsese_zimmer::song::{InlineOnly, PatchRef, Pattern, Song, Track};
use scorsese_zimmer::{Bake, bake_song};

fn tone() -> Patch {
    Patch {
        layers: Vec::new(),
        source: Source::OscStack {
            oscs: vec![osc(Wave::Sine, 0.0, 0)],
        },
        amp: adsr(0.0, 0.0, 1.0, 0.0),
        filter: None,
        pitch_env: None,
        lfo: None,
        lfo2: None,
        fx: vec![],
    }
}

pub(super) fn duet() -> Song {
    let track = |name: &str, gain: f32| Track {
        name: name.to_owned(),
        patch: PatchRef::Inline(Box::new(tone())),
        gain,
        pan: 0.0,
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
                note("sub", "E1", 0.0, 2.0),
                note("bell", "E5", 0.0, 2.0),
            ]),
        },
    );
    Song {
        bpm: 120.0,
        tempo: vec![],
        seed: 7,
        key: None,
        tracks: vec![track("sub", 1.0), track("bell", 1.0)],
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

pub(super) fn bake(song: &Song) -> Bake {
    bake_song(song, &InlineOnly).expect("the fixture song renders")
}

fn row<'a>(tracks: &'a [Layer], name: &str) -> &'a Layer {
    tracks
        .iter()
        .find(|layer| layer.name == name)
        .unwrap_or_else(|| panic!("no row for {name}"))
}

pub(super) fn mean(tracks: &[Layer], name: &str) -> f64 {
    row(tracks, name)
        .level
        .loudness
        .mean_dbfs
        .expect("the track played")
}

#[test]
fn each_track_is_measured_where_its_energy_sits() {
    let tracks = bake(&duet()).tracks;
    assert_eq!(tracks.len(), 2, "one row per track, in track order");
    assert_eq!(tracks[0].name, "sub");
    let low = |name| row(&tracks, name).level.bands.expect("it played").low;
    assert!(low("sub") > 0.9, "a 41 Hz tone is bass: {}", low("sub"));
    assert!(low("bell") < 0.1, "a 659 Hz tone is not: {}", low("bell"));
}

#[test]
fn a_row_is_what_the_track_contributes_after_its_gain() {
    let loud = bake(&duet()).tracks;
    let mut quieter = duet();
    quieter.tracks[0].gain = 0.5;
    let faded = bake(&quieter).tracks;

    let moved = mean(&loud, "sub") - mean(&faded, "sub");
    assert!(
        (moved - 6.02).abs() < 0.1,
        "halving a fader is 6 dB down, not {moved}"
    );
    assert!(
        (mean(&loud, "bell") - mean(&faded, "bell")).abs() < 0.01,
        "the other track did not move"
    );
}

#[test]
fn a_track_that_plays_for_half_the_piece_reads_half_as_present() {
    let throughout = bake(&duet()).tracks;
    let mut briefly = duet();
    briefly.patterns.get_mut("verse").expect("verse").notes = played(vec![
        note("sub", "E1", 0.0, 2.0),
        note("bell", "E5", 0.0, 1.0),
    ]);
    let cut_short = bake(&briefly).tracks;

    let moved = mean(&throughout, "bell") - mean(&cut_short, "bell");
    assert!(
        (moved - 3.01).abs() < 0.2,
        "half the window is 3 dB down, not {moved}"
    );
    assert!(
        (mean(&throughout, "sub") - mean(&cut_short, "sub")).abs() < 0.01,
        "the track that kept playing did not move"
    );
}

#[test]
fn a_track_that_never_plays_is_reported_silent() {
    let mut absent = duet();
    absent.patterns.get_mut("verse").expect("verse").notes =
        played(vec![note("sub", "E1", 0.0, 2.0)]);
    let tracks = bake(&absent).tracks;
    assert_eq!(tracks.len(), 2, "a silent track is still a track");
    assert!(row(&tracks, "bell").level.loudness.is_silent());
}

#[test]
fn a_song_of_one_track_gets_no_rows() {
    let mut alone = duet();
    alone.tracks.truncate(1);
    alone.patterns.get_mut("verse").expect("verse").notes =
        played(vec![note("sub", "E1", 0.0, 2.0)]);
    assert!(bake(&alone).tracks.is_empty());
}

#[test]
fn a_patch_bake_has_no_rows() {
    let opts = crate::common::opts(0.2);
    let bake = scorsese_zimmer::bake_note(&tone(), 60.0, &opts).expect("the patch bakes");
    assert!(bake.tracks.is_empty());
}
