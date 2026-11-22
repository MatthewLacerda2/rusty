//! `humanize`: each pass played afresh in timing, velocity and tone, the first beat kept.

use crate::common::songs::song;
use crate::common::{peak, saw_patch};
use rusty_zimmer::patch::{Adsr, Filter, FilterKind, Slope};
use rusty_zimmer::song::{Humanize, PatchRef};
use rusty_zimmer::{Song, SAMPLE_RATE};

use super::render;

/// A saw, not the stock noise: noise differs pass to pass by design and would hide what moved.
fn steady() -> Song {
    let mut steady = song();
    steady.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
    steady
}

fn pass(song: &Song) -> usize {
    (2.0 * song.beat_seconds() * SAMPLE_RATE as f32) as usize
}

fn onset(pass: &[f32]) -> usize {
    pass.iter()
        .position(|sample| sample.abs() > 0.01)
        .expect("the pass has a note in it")
}

/// Read where the note lands, not its samples: each note's oscillators start at a phase of its own.
#[test]
fn a_pattern_played_twice_is_humanised_differently_each_time() {
    let one = pass(&steady());
    let rigid = render(&steady());
    assert_eq!(
        onset(&rigid[..one]),
        onset(&rigid[one..2 * one]),
        "a song with no feel plays both passes on the same sample"
    );

    let played = render(&Song {
        humanize: Some(Humanize {
            timing: 0.02,
            velocity: 0.1,
            ..Humanize::default()
        }),
        ..steady()
    });
    assert_ne!(
        onset(&played[..one]),
        onset(&played[one..2 * one]),
        "the second pass through a pattern must be played afresh"
    );
}

#[test]
fn a_repeated_note_is_not_the_same_waveform_twice() {
    let one = pass(&steady());
    let rigid = render(&steady());
    assert_ne!(
        rigid[..one],
        rigid[one..2 * one],
        "the second pass is a copy of the first"
    );
    assert_eq!(
        rigid,
        render(&steady()),
        "and the piece still replays exactly"
    );
}

fn expressive() -> Song {
    let mut patch = saw_patch();
    patch.filter = Some(Filter {
        kind: FilterKind::Lowpass,
        slope: Slope::Db12,
        cutoff: 400.0,
        resonance: 0.0,
        env_octaves: 0.0,
        vel_octaves: 3.5,
        key_track: 0.0,
        adsr: Adsr::default(),
    });
    let mut expressive = steady();
    expressive.tracks[0].patch = PatchRef::Inline(Box::new(patch));
    expressive
}

#[test]
fn a_timbre_amount_moves_the_tone_and_leaves_the_timing_alone() {
    let one = pass(&expressive());
    let rigid = render(&expressive());
    let played = render(&Song {
        humanize: Some(Humanize {
            timbre: 0.4,
            ..Humanize::default()
        }),
        ..expressive()
    });
    assert_eq!(
        onset(&rigid[..one]),
        onset(&played[..one]),
        "a tone nudge moved a note off its beat"
    );
    assert_ne!(rigid, played, "the touch never reached the instrument");
}

#[test]
fn an_early_nudge_on_the_very_first_beat_still_sounds() {
    let window = (0.6 * SAMPLE_RATE as f32) as usize;
    for seed in 0..8 {
        let wild = Song {
            seed,
            humanize: Some(Humanize {
                timing: 0.5,
                ..Humanize::default()
            }),
            ..steady()
        };
        let rendered = render(&wild);
        assert!(
            rendered.len() >= 2 * pass(&wild),
            "the song kept its length"
        );
        assert!(
            peak(&rendered[..window]) > 0.0,
            "seed {seed} lost the first note off the front"
        );
    }
}
