//! What every articulation test is written against.
//!
//! The instrument is a **saw through a low-pass a hard strike opens**, held
//! flat and released instantly. That makes all three of the things a mark
//! moves measurable from the samples themselves: velocity is the peak level,
//! the velocity-to-brightness routing is the tone, and the gate is exactly how
//! long the note sounds. Asserting what a mark *did* rather than that a field
//! parsed is the whole point of the file.

use crate::common::songs::{song, verse};
use crate::common::{channel, saw_patch};
use scorsese_zimmer::patch::{Adsr, Filter, FilterKind, Patch, Slope};
use scorsese_zimmer::song::{Articulation, InlineOnly, Note, PatchRef, PatternEntry, Pitch};
use scorsese_zimmer::{SAMPLE_RATE, Song, render_song};

/// The track every fixture note is on.
pub(crate) const TRACK: &str = "bass";

/// How coarsely [`sounding`] measures, in samples — one block of the envelope
/// it walks. Comfortably finer than anything asserted against it: a ghost sits
/// 529 samples early and a gate under test is twelve thousand long.
pub(crate) const BLOCK: usize = 64;

/// The instrument: a saw whose cutoff a full-velocity strike opens by four
/// octaves, so a change in tone that is not a change in level still shows.
pub(crate) fn instrument() -> Patch {
    Patch {
        filter: Some(Filter {
            kind: FilterKind::Lowpass,
            slope: Slope::Db12,
            cutoff: 300.0,
            resonance: 0.0,
            env_octaves: 0.0,
            vel_octaves: 4.0,
            key_track: 0.0,
            adsr: Adsr::default(),
        }),
        ..saw_patch()
    }
}

/// One note, with a mark over it or without one.
pub(crate) fn note(name: &str, start: f32, dur: f32, vel: f32, mark: Option<Articulation>) -> Note {
    Note {
        track: TRACK.to_owned(),
        note: Pitch::Name(name.to_owned()),
        start,
        dur,
        vel,
        articulation: mark,
    }
}

/// The fixture playing `entries` once through a four-beat pattern, in E minor
/// — the key a degree needs and every other form ignores.
///
/// The gain is low and so are the velocities the tests write, because the
/// master limiter is not optional: a peak that reached it would answer a
/// question about the limiter under the name of an articulation.
pub(crate) fn playing(entries: Vec<PatternEntry>) -> Song {
    let mut one_pass = song();
    one_pass.arrangement = vec!["verse".into()];
    one_pass.key = Some("E minor".to_owned());
    one_pass.tracks[0].patch = PatchRef::Inline(Box::new(instrument()));
    one_pass.tracks[0].gain = 0.4;
    let pattern = verse(&mut one_pass);
    pattern.beats = 4.0;
    pattern.notes = entries;
    one_pass
}

/// The left channel of `song`, rendered — one waveform is the whole answer to
/// every question here, none of which is about width.
pub(crate) fn rendered(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

/// The first and last sample at which the note is sounding.
///
/// Walked as an envelope of block maxima rather than sample by sample, because
/// an oscillator starts somewhere in its cycle: the first sample after the gate
/// opens can be anywhere between zero and full scale, and reading it directly
/// would measure the phase instead of the onset.
pub(crate) fn sounding(buf: &[f32]) -> (usize, usize) {
    let blocks: Vec<f32> = buf
        .chunks(BLOCK)
        .map(|block| block.iter().fold(0.0f32, |most, x| most.max(x.abs())))
        .collect();
    let loudest = blocks.iter().fold(0.0f32, |most, x| most.max(*x));
    let open = |index: &usize| blocks[*index] > loudest * 0.1;
    let first = (0..blocks.len()).find(open).expect("the note sounds");
    let last = (0..blocks.len()).rfind(open).expect("the note sounds");
    (first * BLOCK, (last + 1) * BLOCK)
}

/// How long the note sounded, in samples.
pub(crate) fn gate(buf: &[f32]) -> usize {
    let (first, last) = sounding(buf);
    last - first
}

/// Seconds as a sample count, for an assertion written in the units the
/// documentation is.
pub(crate) fn samples(seconds: f32) -> f32 {
    seconds * SAMPLE_RATE as f32
}
