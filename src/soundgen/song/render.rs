//! src/soundgen/song/render.rs — walk the arrangement, sum the notes, bake (#358).
//!
//! The whole renderer is one idea: **mixing is addition**. Every note is rendered
//! independently through the #357 note-renderer, and its buffer is added into the
//! master at its start offset. There is no streaming, no voice allocation and no
//! voice-stealing, because none of that buys anything at bake time — this is not a
//! real-time synth, it is a file being written, and memory is cheap.
//!
//! Two deliberate consequences:
//!
//! - **Nothing is cut off.** The buffer grows to fit whatever the last note's release
//!   and fx tail need, so a song ends by ringing out rather than by a hard stop at the
//!   final beat.
//! - **Notes are rendered *unlimited*** and only the finished mix is limited. The
//!   per-note limiter [`render_limited`](crate::soundgen::render_limited) exists so a
//!   single baked one-shot can't clip its own file; applying it here as well would
//!   squash each note's dynamics *before* the mix, then squash the sum again. The
//!   master limiter is the guarantee that matters, and it is not optional.
//!
//! **Determinism.** Every note's seed is derived from `(song.seed, track index, note
//! ordinal)` through the same seeded integer hash `procgen` established — no `rand`,
//! no wall clock. The ordinal counts notes in arrangement order, so a pattern played
//! twice gets two different noise draws (a repeated snare shouldn't be a photocopy)
//! while the whole piece stays byte-identical across runs and processes.

use std::collections::HashMap;

use super::{PatchRef, Song};
use crate::procgen::hash::hash3;
use crate::soundgen::core::{self, RATE, SAMPLE_RATE};
use crate::soundgen::fx::limiter;
use crate::soundgen::note::NoteOpts;
use crate::soundgen::patch::Patch;
use crate::soundgen::wav;

/// Render `song` to a mono sample buffer at [`SAMPLE_RATE`], master-limited.
pub fn render_song(song: &Song) -> Result<Vec<f32>, String> {
    song.validate()?;
    let patches = resolve_patches(song)?;
    let track_index: HashMap<&str, usize> = song
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.as_str(), i))
        .collect();

    let beat = song.beat_seconds();
    // Start at the arrangement's own length rather than growing from empty, so a song
    // whose last pattern ends on a rest keeps that rest. Truncating to the final
    // *note* instead would silently shorten the file and put a loop point in the
    // wrong place — the arrangement is the score, not just where the samples happen
    // to stop. Tails then extend it past this (see `mix_into`).
    let mut master = vec![0.0f32; (song.arrangement_beats() * beat * RATE).round() as usize];
    let mut cursor_beats = 0.0f32;
    let mut ordinal: u64 = 0;

    for name in &song.arrangement {
        let pattern = song
            .patterns
            .get(name)
            .ok_or_else(|| format!("arrangement names pattern `{name}`, which is not defined"))?;
        for note in &pattern.notes {
            let track = track_index[note.track.as_str()];
            let opts = NoteOpts {
                duration: note.dur * beat,
                velocity: note.vel,
                seed: note_seed(song.seed, track, ordinal),
            };
            ordinal += 1;
            let rendered = core::render(&patches[track], note.note.to_midi()?, &opts)?;
            let at = ((cursor_beats + note.start) * beat * RATE).round() as usize;
            mix_into(&mut master, &rendered, at, song.tracks[track].gain);
        }
        cursor_beats += pattern.beats;
    }

    // The master fx, always — mixing by addition is exactly the operation that
    // overshoots full scale, so the sum is never handed out unlimited.
    limiter::apply(&mut master, RATE);
    Ok(master)
}

/// Render `song` and write it as a mono 16-bit PCM WAV at `path`. Returns the path.
pub fn bake_song(song: &Song, path: &str) -> Result<String, String> {
    let samples = render_song(song)?;
    wav::write(&samples, SAMPLE_RATE, path)
}

/// Add `src * gain` into `master` starting at sample `at`, growing `master` to fit.
///
/// The growth is what lets the song ring out: the last note's release tail extends
/// the buffer past the final beat instead of being truncated to it.
fn mix_into(master: &mut Vec<f32>, src: &[f32], at: usize, gain: f32) {
    let end = at + src.len();
    if master.len() < end {
        master.resize(end, 0.0);
    }
    for (dst, s) in master[at..end].iter_mut().zip(src) {
        *dst += s * gain;
    }
}

/// One seed per note, from `(song seed, track, ordinal)`.
///
/// Two 32-bit hashes on different channels are stitched into the `u64` the note
/// renderer wants, so the full seed space is used rather than the low 32 bits.
fn note_seed(song_seed: u64, track: usize, ordinal: u64) -> u64 {
    let hi = hash3(track as i64, ordinal as i64, 0, song_seed) as u64;
    let lo = hash3(track as i64, ordinal as i64, 1, song_seed) as u64;
    (hi << 32) | lo
}

/// Load every track's patch once, up front.
///
/// Up front rather than per note because a song is thousands of notes over a handful
/// of instruments: resolving inside the loop would re-read and re-parse the same
/// `.json` once per note. Failing here also means a missing patch file is reported
/// before any rendering happens.
fn resolve_patches(song: &Song) -> Result<Vec<Patch>, String> {
    song.tracks
        .iter()
        .map(|track| match &track.patch {
            PatchRef::Inline(patch) => Ok((**patch).clone()),
            PatchRef::Path(path) => {
                let json = std::fs::read_to_string(path).map_err(|e| {
                    format!("track `{}`: cannot read patch `{path}`: {e}", track.name)
                })?;
                Patch::from_json(&json)
                    .map_err(|e| format!("track `{}`: patch `{path}` is invalid: {e}", track.name))
            }
        })
        .collect()
}
