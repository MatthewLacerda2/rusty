//! A compressor where one belongs, and a kick pressing a pad out of the way.

use crate::common::songs::{note, song, verse};
use crate::common::{channel, peak, saw_patch};
use rusty_zimmer::patch::Fx;
use rusty_zimmer::song::{InlineOnly, PatchRef, Song, Track};
use rusty_zimmer::{render_song, SynthError};

const RATE: f32 = 44_100.0;

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

fn compressor(sidechain: Option<&str>) -> Fx {
    Fx::Compress {
        threshold: -30.0,
        ratio: 8.0,
        attack: 0.005,
        release: 0.1,
        makeup: 0.0,
        mix: 1.0,
        sidechain: sidechain.map(str::to_owned),
    }
}

fn under_a_silent_kick(chain: Vec<Fx>) -> Song {
    let mut song = song();
    song.tracks[0].gain = 0.0;
    song.tracks.push(Track {
        name: "pad".to_owned(),
        patch: PatchRef::Inline(Box::new(saw_patch())),
        gain: 0.8,
        pan: 0.0,
        fx: chain,
        send: 1.0,
        push: 0.0,
        swing: None,
        humanize: None,
    });
    verse(&mut song)
        .notes
        .push(note("pad", "E3", 0.0, 2.0).into());
    song
}

fn peak_between(mix: &[f32], from: f32, to: f32) -> f32 {
    let left = channel(mix, 0);
    let (from, to) = (
        (from * RATE) as usize,
        ((to * RATE) as usize).min(left.len()),
    );
    peak(&left[from..to])
}

#[test]
fn a_kick_presses_the_pad_down_on_every_beat_and_lets_it_back() {
    // The third beat, and the gap before the fourth: far enough into the piece that neither is the
    // first note starting.
    let (under, between) = ((1.5, 1.56), (1.85, 1.95));
    let ducked = render(&under_a_silent_kick(vec![compressor(Some("bass"))]));
    let hit = peak_between(&ducked, under.0, under.1);
    let gap = peak_between(&ducked, between.0, between.1);
    assert!(
        hit < gap * 0.5,
        "the pad did not move under the kick: {hit} against {gap}"
    );

    let flat = render(&under_a_silent_kick(Vec::new()));
    let steady = peak_between(&flat, under.0, under.1) / peak_between(&flat, between.0, between.1);
    assert!(
        (steady - 1.0).abs() < 0.1,
        "the pad is only steady if the duck was the compressor: {steady}"
    );
}

#[test]
fn the_same_compressor_with_no_key_does_not_follow_the_beat() {
    let alone = render(&under_a_silent_kick(vec![compressor(None)]));
    let ratio = peak_between(&alone, 1.5, 1.56) / peak_between(&alone, 1.85, 1.95);
    assert!(
        (ratio - 1.0).abs() < 0.1,
        "something other than the key moved this gain: {ratio}"
    );
}

#[test]
fn a_sidechain_has_to_name_another_track_of_this_song() {
    let missing = under_a_silent_kick(vec![compressor(Some("nobody"))]);
    assert_eq!(
        missing.validate(),
        Err(SynthError::UnknownSidechain {
            track: "pad".to_owned(),
            key: "nobody".to_owned(),
        })
    );
    let itself = under_a_silent_kick(vec![compressor(Some("pad"))]);
    assert_eq!(
        itself.validate(),
        Err(SynthError::SelfSidechain {
            track: "pad".to_owned(),
        })
    );
}

#[test]
fn only_a_track_chain_sits_where_one_part_can_listen_to_another() {
    let misplaced = |place| {
        Err(SynthError::MisplacedSidechain {
            place,
            key: "bass".to_owned(),
        })
    };
    let on_the_sum = Song {
        fx: vec![compressor(Some("bass"))],
        ..under_a_silent_kick(Vec::new())
    };
    assert_eq!(on_the_sum.validate(), misplaced("song"));

    let mut instrument = saw_patch();
    instrument.fx = vec![compressor(Some("bass"))];
    assert_eq!(instrument.validate(), misplaced("patch"));
}

#[test]
fn makeup_on_the_sum_never_gets_past_the_limiter() {
    let plain = render(&under_a_silent_kick(Vec::new()));
    let glue = |makeup| Song {
        fx: vec![Fx::Compress {
            threshold: -30.0,
            ratio: 4.0,
            attack: 0.01,
            release: 0.1,
            makeup,
            mix: 1.0,
            sidechain: None,
        }],
        ..under_a_silent_kick(Vec::new())
    };
    let (glued, unmade) = (render(&glue(24.0)), render(&glue(0.0)));
    assert!(peak(&glued) <= 0.98 + 1e-6, "peaked at {}", peak(&glued));
    let (before, after) = (
        peak_between(&unmade, 1.85, 1.95),
        peak_between(&glued, 1.85, 1.95),
    );
    assert!(
        after > before * 4.0,
        "and it is louder for it: {before} → {after}"
    );
    assert_eq!(glued.len(), plain.len(), "nothing was added to ring out");
}
