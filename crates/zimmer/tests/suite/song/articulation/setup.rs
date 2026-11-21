//! What every articulation test is written against.

use crate::common::songs::{song, verse};
use crate::common::{channel, saw_patch};
use scorsese_zimmer::patch::{Adsr, Filter, FilterKind, Patch, Slope};
use scorsese_zimmer::song::{Articulation, InlineOnly, Note, PatchRef, PatternEntry, Pitch};
use scorsese_zimmer::{SAMPLE_RATE, Song, render_song};

pub(crate) const TRACK: &str = "bass";

pub(crate) const BLOCK: usize = 64;

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

pub(crate) fn rendered(song: &Song) -> Vec<f32> {
    channel(
        &render_song(song, &InlineOnly).expect("the song renders"),
        0,
    )
}

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

pub(crate) fn gate(buf: &[f32]) -> usize {
    let (first, last) = sounding(buf);
    last - first
}

pub(crate) fn samples(seconds: f32) -> f32 {
    seconds * SAMPLE_RATE as f32
}
