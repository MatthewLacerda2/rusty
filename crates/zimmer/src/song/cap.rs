//! The longest a song may ask to be rendered: [`MAX_SONG_SECONDS`].
//!
//! A song is rendered into one buffer, allocated whole, so its length is its
//! memory: nothing else in a document stops a pattern of a million beats, a
//! `loop` fit a million seconds long, or a note placed a million beats in.
//! [`Song::validate`] refuses each before a sample exists, by measuring the
//! buffer the render allocates: the passes the render plays (`shape::plan`)
//! at the tempo it plays them, the last pass as far as its last note reaches,
//! and the target of a `fit` when it pads past them. A `loop` renders whole
//! passes and is cut to its target afterwards, so it is measured by those
//! passes: 7 s passes looped to 600 s render 602 s, and are refused.
//!
//! A note's own release and its effect tail are already bounded where a note
//! is (`NoteTooLong`), so they are not counted here.

use std::collections::HashMap;

use super::Song;
use super::key::Key;
use super::shape::{fitted, plan};
use crate::error::SynthError;

/// The longest a song may be rendered, in seconds: ten minutes, longer than
/// any bed a video plays under one scene, and well short of a buffer that
/// hurts the machine it is baked on.
pub const MAX_SONG_SECONDS: f32 = 600.0;

/// Refused when `song`, read in `key`, would render past
/// [`MAX_SONG_SECONDS`]. Called last by `validate`, on a song otherwise valid.
pub(super) fn check(song: &Song, key: Option<&Key>) -> Result<(), SynthError> {
    let (clock, passes) = plan(song);
    // The last pass reaches at least to its own end: each slot's furthest
    // layer is at least as long as the slot.
    let before = song.arrangement_beats() * passes.saturating_sub(1) as f32;
    let reached = clock.seconds(before + last_beat(song, key)?);
    let seconds = fitted(song).map_or(reached, |(_, target)| reached.max(target));
    if seconds.is_finite() && seconds <= MAX_SONG_SECONDS {
        return Ok(());
    }
    Err(SynthError::SongTooLong {
        seconds,
        limit: MAX_SONG_SECONDS,
    })
}

/// The beat the last thing in one pass ends on: each slot as far as its
/// furthest-reaching layer reaches.
fn last_beat(song: &Song, key: Option<&Key>) -> Result<f32, SynthError> {
    let mut reach = HashMap::with_capacity(song.patterns.len());
    for (name, pattern) in &song.patterns {
        let notes = pattern.voices(key)?;
        let end = notes
            .iter()
            .map(|note| note.start + note.dur)
            .fold(pattern.beats, f32::max);
        reach.insert(name.as_str(), end);
    }
    let (mut cursor, mut end) = (0.0f32, 0.0f32);
    for entry in &song.arrangement {
        for layer in entry.layers() {
            if let Some(reaches) = reach.get(layer.pattern()) {
                end = end.max(cursor + reaches);
            }
        }
        cursor += song.slot_beats(entry).unwrap_or(0.0);
    }
    Ok(end)
}
