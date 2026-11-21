//! A compressor where one belongs, and a kick pressing a pad out of the way.

use crate::common::songs::{note, song, verse};
use crate::common::{channel, peak, saw_patch};
use scorsese_zimmer::patch::Fx;
use scorsese_zimmer::song::{InlineOnly, PatchRef, Song, Track};
use scorsese_zimmer::{SynthError, render_song};

const RATE: f32 = 44_100.0;

fn render(song: &Song) -> Vec<f32> {
    render_song(song, &InlineOnly).expect("the song renders")
}

/// A hard, fast compressor, keyed from `sidechain` when there is one.
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

/// The fixture's blip track on every beat, silenced at its fader, under a
/// held saw carrying `chain`.
///
/// The saw is [`saw_patch`] — a fully sustaining envelope, so it holds one
/// level for as long as the note lasts and anything that moves it is the
/// compressor rather than the playing.
///
/// The key track's `gain` is **zero**, so nothing it plays reaches the mix.
/// Whatever the pad does at those moments is therefore the sidechain and
/// cannot be the kick being audible — and it is the pre-fader rule asserted
/// rather than described.
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

/// The loudest the finished mix gets between two moments of it, in seconds.
fn peak_between(mix: &[f32], from: f32, to: f32) -> f32 {
    let left = channel(mix, 0);
    let (from, to) = (
        (from * RATE) as usize,
        ((to * RATE) as usize).min(left.len()),
    );
    peak(&left[from..to])
}

/// The move the issue is about. The kick lands on every beat; the pad is a
/// steady note that never changes level on its own. With the sidechain the pad
/// is pushed out of the way on each hit and recovers by the next one, and
/// without any chain at all those two moments are the same level.
#[test]
fn a_kick_presses_the_pad_down_on_every_beat_and_lets_it_back() {
    // The third beat, and the gap before the fourth: far enough into the piece
    // that neither is the first note starting.
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

/// The same compressor with nothing to listen to is not a duck: it reads the
/// pad's own level, which does not move, so neither does its gain.
#[test]
fn the_same_compressor_with_no_key_does_not_follow_the_beat() {
    let alone = render(&under_a_silent_kick(vec![compressor(None)]));
    let ratio = peak_between(&alone, 1.5, 1.56) / peak_between(&alone, 1.85, 1.95);
    assert!(
        (ratio - 1.0).abs() < 0.1,
        "something other than the key moved this gain: {ratio}"
    );
}

/// A name that is not a track, and a track naming itself: both the typo that
/// would otherwise be a duck the recipe wrote and never heard.
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

/// The other two chain locations have no track to listen to, so they refuse a
/// key rather than dropping it quietly.
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

/// Glue on the sum, which is the other half of what this is for — and the
/// constraint that comes with it: a song's chain runs *before* the limiter, so
/// makeup gain asking for 24 dB does not get a mix past the ceiling. The piece
/// does not get longer either: a compressor scales samples and adds none.
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
    // Over a steady moment rather than the whole piece, against the same
    // compressor with no makeup: the plain pad already sits at the limiter's
    // ceiling, and a limiter that leaves the waveform's shape alone cannot
    // make a steady tone louder than that.
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
