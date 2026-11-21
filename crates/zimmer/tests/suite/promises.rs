//! The two claims the rest of scorsese is built on: a bake is **deterministic**,
//! and a note comes out at the **pitch it was asked for**.
//!
//! Everything else here is a detail of how a sound is made. These two are the
//! contract: `generated/` addresses a bake by the hash of its recipe, which is
//! only sound if the same recipe really does produce the same bytes.

use crate::common;

use common::songs::song;
use common::{measured_hz, minimal, noise, opts, saw_patch};
use scorsese_zimmer::patch::{Adsr, Osc, Patch, Source, Wave};
use scorsese_zimmer::song::{Humanize, InlineOnly};
use scorsese_zimmer::{NoteOpts, Song, bake_named_note, bake_note, bake_song};

/// A bare sine with no envelope movement, so the spacing of its rising
/// zero-crossings *is* the frequency and the pitch can be asserted without an
/// FFT.
fn sine() -> Patch {
    Patch {
        amp: Adsr {
            a: 0.0,
            d: 0.0,
            s: 1.0,
            r: 0.0,
            curve: 0.0,
        },
        ..minimal(Source::OscStack {
            oscs: vec![Osc {
                wave: Wave::Sine,
                detune_cents: 0.0,
                gain: 1.0,
                octave: 0,
                voices: 1,
                spread: 12.0,
                width: 0.0,
                pulse_width: 0.5,
            }],
        })
    }
}

#[test]
fn a_note_lands_on_the_pitch_it_was_asked_for() {
    let one_second = NoteOpts {
        duration: 1.0,
        velocity: 1.0,
        timbre: 0.0,
        glide: None,
        seed: 0,
    };
    let hz = |midi: f32| {
        let buf = common::render(&sine(), midi, &one_second);
        measured_hz(&buf[..44_100], 44_100.0)
    };
    let close = |midi: f32, expected: f32| {
        let measured = hz(midi);
        assert!(
            (measured - expected).abs() < 0.5,
            "asked for {expected} Hz, got {measured}"
        );
    };
    close(69.0, 440.0);
    close(57.0, 220.0);
    close(81.0, 880.0);
}

/// The limiter's promise, checked with the meter that reports on the same
/// bake — because a guarantee and a measurement that disagree are worse than
/// either alone.
///
/// Judged on the **true** peak, which is what a converter or a lossy encoder
/// has to reproduce. An ordinary song is the case worth asserting on rather
/// than a contrived one: this one's loudest *sample* sits nearly two decibels
/// under the ceiling, so the limiter barely engages, and the waveform between
/// its samples went past full scale anyway.
#[test]
fn a_bake_does_not_clip() {
    let song = bake_song(&song(), &InlineOnly).expect("bakes");
    assert!(
        !song.loudness().is_clipping(),
        "the song clips: {:?}",
        song.loudness()
    );
    let note = bake_note(&saw_patch(), 93.0, &opts(0.3)).expect("bakes");
    assert!(
        !note.loudness().is_clipping(),
        "the one-shot clips: {:?}",
        note.loudness()
    );
}

/// The claim `generated/` rests on. If this ever fails, a cache hit is serving
/// bytes the recipe no longer describes.
#[test]
fn the_same_recipe_bakes_the_same_bytes() {
    let patch = saw_patch();
    let first = bake_note(&patch, 60.0, &opts(0.3)).expect("bakes");
    let second = bake_note(&patch, 60.0, &opts(0.3)).expect("bakes");
    assert_eq!(
        first, second,
        "a bake must be a pure function of its inputs"
    );
}

/// A stochastic source is the interesting case: noise that is *seeded* still
/// has to be reproducible, and a different seed still has to differ.
#[test]
fn seeded_noise_is_reproducible_and_a_new_seed_is_not() {
    let noise = minimal(noise());
    let seeded =
        |seed: u64| bake_note(&noise, 60.0, &NoteOpts { seed, ..opts(0.2) }).expect("bakes");
    assert_eq!(seeded(11), seeded(11));
    assert_ne!(seeded(11), seeded(12));
}

/// A song with a *feel* is the case that could quietly break this: "played,
/// not clocked" is the one place a clock or an `rand` would be tempting, and
/// either would put an asterisk on everything above.
#[test]
fn a_humanised_song_bakes_the_same_bytes_and_a_new_seed_does_not() {
    let played = |seed: u64| {
        let performance = Song {
            seed,
            swing: 0.3,
            humanize: Some(Humanize {
                timing: 0.02,
                velocity: 0.1,
                timbre: 0.08,
            }),
            ..song()
        };
        bake_song(&performance, &InlineOnly).expect("bakes")
    };
    assert_eq!(
        played(4),
        played(4),
        "a performance is still a pure function"
    );
    assert_ne!(played(4), played(5), "and the seed still re-rolls it");
}

/// Absent means what it meant before the fields existed — which is what keeps
/// every bake already in a `generated/` directory valid.
#[test]
fn asking_for_no_feel_bakes_what_a_song_that_never_heard_of_it_does() {
    let plain = song();
    assert_eq!(plain.swing, 0.0);
    assert!(plain.humanize.is_none());
    let spelled_out = Song {
        humanize: Some(Humanize::default()),
        ..song()
    };
    assert_eq!(
        bake_song(&spelled_out, &InlineOnly).expect("bakes"),
        bake_song(&plain, &InlineOnly).expect("bakes"),
    );
}

#[test]
fn naming_a_note_and_numbering_it_bake_the_same_file() {
    let patch = saw_patch();
    let named = bake_named_note(&patch, "C#4", &opts(0.2)).expect("bakes by name");
    let numeric = bake_note(&patch, 61.0, &opts(0.2)).expect("bakes by number");
    assert_eq!(named, numeric);
}

#[test]
fn an_unknown_note_name_is_rejected_by_name() {
    let refused = bake_named_note(&saw_patch(), "H4", &opts(0.2)).expect_err("H is not a note");
    assert!(refused.to_string().contains("H4"), "got {refused}");
}
