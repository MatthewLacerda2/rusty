//! Every note a song sounds, with where it sounds — the one walk the survey's
//! reports are all read from.
//!
//! The renderer's own expansion, step for step: each entry voiced through
//! [`PatternEntry::voice_into`], each layer's `tracks` filter applied, each
//! pitch moved by [`Layer::played_pitch`]. What it leaves out is everything
//! that only decides *how* a note is played (swing, humanise, articulation),
//! since no report here reads a performance.

use crate::song::chord::Led;
use crate::song::{Context, PatternEntry, Song, Steps};

/// One note as the arrangement plays it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Sounded {
    /// The track playing it, as the note names it.
    pub(super) track: String,
    /// Where it sounds in MIDI after the layer's transposes, or nothing when
    /// its name does not parse — still a note the track holds the time for.
    pub(super) midi: Option<f32>,
    /// Its onset in beats from the top of one pass through the arrangement.
    pub(super) start: f32,
    /// Its written length in beats.
    pub(super) dur: f32,
    /// Which arrangement entry plays it, by index.
    pub(super) entry: usize,
    /// Where that entry's slot starts, in beats.
    pub(super) slot: f32,
    /// Whether the page states its pitch. A step string with no `note` plays
    /// its default without stating one, which is how percussion is written.
    pub(super) stated: bool,
}

/// Calls `each` with every note one pass of `song` sounds, in arrangement
/// order.
///
/// Tolerant the way every report is: an entry that does not expand (a chord
/// off the table, a degree under no key) costs that entry and not the pattern
/// around it, and an arrangement entry naming no pattern is skipped. Refusing
/// the document is [`Song::validate`]'s job.
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
        // Where the *slot* ends, not where its sound does — the next pattern
        // starts on time however long the last note rings.
        at += slot;
    }
}
