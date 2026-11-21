//! The longest a song may ask to be rendered: [`MAX_SONG_SECONDS`].

use rusty_core::collections::Map;

use super::key::Key;
use super::shape::{fitted, plan};
use super::Song;
use crate::error::SynthError;

pub const MAX_SONG_SECONDS: f32 = 600.0;

pub(super) fn check(song: &Song, key: Option<&Key>) -> Result<(), SynthError> {
    let (clock, passes) = plan(song);
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

fn last_beat(song: &Song, key: Option<&Key>) -> Result<f32, SynthError> {
    let mut reach = Map::with_capacity_and_hasher(song.patterns.len(), Default::default());
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
