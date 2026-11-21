//! `swing` and `humanize`: a song played rather than clocked.

use crate::common::songs::{song, verse, voice};
use crate::common::{channel, peak, saw_patch};
use scorsese_zimmer::patch::{Adsr, Filter, FilterKind, Slope};
use scorsese_zimmer::song::{Humanize, InlineOnly, PatchRef};
use scorsese_zimmer::{SAMPLE_RATE, Song, render_song};

/// Renders a song whose instruments are all inline, as one channel of it —
/// a performance is a matter of when and how hard, not of where.
fn render(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

/// The fixture with a *deterministic* instrument.
///
/// The stock one is noise, which already differs from pass to pass by design
/// and would hide the thing these tests are about. With a plain saw, two
/// passes of one pattern are bit-identical unless something moved them apart.
fn steady() -> Song {
    let mut steady = song();
    steady.tracks[0].patch = PatchRef::Inline(Box::new(saw_patch()));
    steady
}

/// How many samples one pass of the two-beat fixture pattern takes.
fn pass(song: &Song) -> usize {
    (2.0 * song.beat_seconds() * SAMPLE_RATE as f32) as usize
}

/// The strongest form the claim takes: swinging a song equals writing its
/// off-beats late by hand, to the sample.
#[test]
fn swinging_equals_writing_the_off_beats_late() {
    let mut swung = song();
    voice(verse(&mut swung), 1).start = 0.5;
    swung.swing = 0.5;

    let mut by_hand = song();
    voice(verse(&mut by_hand), 1).start = 0.75;

    assert_eq!(render(&swung), render(&by_hand));
}

/// Swing moves the off-beats and nothing else: a note on the beat is left
/// where it was written, which is what makes swing a feel rather than a delay.
#[test]
fn a_note_on_the_beat_does_not_move_however_hard_the_song_swings() {
    let straight = song();
    let on_the_beat = straight.patterns["verse"]
        .notes
        .iter()
        .all(|note| note.start().fract() == 0.0);
    assert!(on_the_beat, "the fixture is written on the beat");

    let swung = Song {
        swing: 0.66,
        ..song()
    };
    assert_eq!(render(&swung), render(&straight));
}

/// Where the first note of a pass sounds, as a sample offset inside it.
///
/// The fixture's saw sustains fully and releases instantly, so a pass holds
/// nothing of the one before it and the first sample with any signal in it is
/// the onset.
fn onset(pass: &[f32]) -> usize {
    pass.iter()
        .position(|sample| sample.abs() > 0.01)
        .expect("the pass has a note in it")
}

/// The point of keying the draw on the note's ordinal rather than on its
/// pattern: eight bars nudged identically both times round is still a
/// photocopy, just a crooked one.
///
/// Measured as *where the note lands* rather than by comparing the samples,
/// because the samples of two passes already differ — each note starts its
/// oscillators somewhere in their cycle, drawn from its own seed — and a
/// comparison that passed on that alone would say nothing about humanising.
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

/// And a repeat is not a photocopy even with no `humanize` written at all: the
/// note the second pass plays is the same note struck a second time, which no
/// instrument has ever answered identically.
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

/// The fixture again, with an instrument that routes velocity at its filter —
/// the only kind of patch `timbre` can reach, and the kind that shows it.
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

/// `timbre` scatters the tone and nothing else: the notes land where the score
/// put them, and what arrives there is played with a different touch each time.
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

/// A note written on beat zero and nudged early has nowhere to go, so it is
/// clamped to the start of the buffer rather than lost off the front of it.
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
