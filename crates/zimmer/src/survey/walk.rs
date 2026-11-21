//! Every note a song sounds, with where it sounds — the one walk the survey's reports are all read
//! from.

use crate::song::chord::Led;
use crate::song::{Context, PatternEntry, Song, Steps};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Sounded {
    pub(super) track: String,
    pub(super) midi: Option<f32>,
    pub(super) start: f32,
    pub(super) dur: f32,
    pub(super) entry: usize,
    pub(super) slot: f32,
    pub(super) stated: bool,
}

pub(super) fn walk(song: &Song, mut each: impl FnMut(Sounded)) {
    let key = song.key().ok().flatten();
    let mut at = 0.0;
    for (index, entry) in song.arrangement.iter().enumerate() {
        let Some(slot) = song.slot_beats(entry) else {
            continue;
        };
        for layer in entry.layers() {
            let Some(pattern) = song.patterns.get(layer.pattern()) else {
                continue;
            };
            let around = Context {
                beats: pattern.beats,
                key: key.as_ref(),
            };
            let mut voiced = Vec::new();
            let mut stated = Vec::new();
            let mut led = Led::default();
            for written in &pattern.notes {
                let _ = written.voice_into(around, &mut led, &mut voiced);
                let says = !matches!(written, PatternEntry::Steps(Steps { note: None, .. }));
                stated.resize(voiced.len(), says);
            }
            for (note, stated) in voiced.into_iter().zip(stated) {
                if !layer.plays(&note.track) {
                    continue;
                }
                each(Sounded {
                    track: note.track,
                    midi: note
                        .note
                        .to_midi()
                        .ok()
                        .map(|written| layer.played_pitch(written, key.as_ref())),
                    start: at + note.start,
                    dur: note.dur,
                    entry: index,
                    slot: at,
                    stated,
                });
            }
        }
        at += slot;
    }
}
