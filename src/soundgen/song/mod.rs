//! src/soundgen/song/ — the song document: patterns, arrangement, tracks (#358).
//!
//! The second half of the #356 audio-authoring layer. Where [`crate::soundgen::patch`]
//! describes *one instrument*, a [`Song`] describes *a piece of music*: which
//! instruments play (tracks), what they play (patterns of notes), and in what order
//! (the arrangement).
//!
//! The shape is **tracker-style** (the MOD/XM lineage), not a linear piano roll, and
//! deliberately so: a Mario-class melody is a handful of short patterns and a list
//! naming them, which is a few dozen lines of text an agent can write, diff and
//! iterate on. A flat note list of the same music would be hundreds of lines with the
//! repetition spelled out, and every edit would be a merge conflict with itself.
//!
//! Everything is **beats**, never seconds — `bpm` converts once at render time — so
//! changing the tempo of a finished song is one number.
//!
//! ```jsonc
//! {
//!   "bpm": 120,
//!   "seed": 7,
//!   "tracks": [{ "name": "bass", "patch": "project/audio/bass.json", "gain": 0.8 }],
//!   "patterns": {
//!     "verse": { "beats": 8, "notes": [
//!       { "track": "bass", "note": "E2", "start": 0.0, "dur": 0.5, "vel": 1.0 }] }
//!   },
//!   "arrangement": ["verse", "verse"]
//! }
//! ```
//!
//! Rendering lives in [`render`]; this file is the document alone — plain serde data
//! that round-trips losslessly, the same "document as truth" rule the patch follows.

pub mod render;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::patch::Patch;

/// Default for a per-track or per-note gain: unity, i.e. "as written".
fn one() -> f32 {
    1.0
}

/// A complete piece of music, renderable to one mono WAV.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Song {
    /// Tempo in beats per minute; the one place beats become seconds.
    pub bpm: f32,
    /// Folded into every note's render seed, so one number re-rolls every stochastic
    /// source in the piece while keeping it reproducible.
    #[serde(default)]
    pub seed: u64,
    /// The instruments, in a fixed order — the order is part of the seed derivation,
    /// so it is data, not presentation.
    pub tracks: Vec<Track>,
    /// Named blocks of notes. A `BTreeMap` rather than a `HashMap` so
    /// [`Song::to_json`] emits patterns in a stable order and two saves of the same
    /// song are byte-identical.
    pub patterns: BTreeMap<String, Pattern>,
    /// Which patterns play, in order. Repeats are just repeats.
    pub arrangement: Vec<String>,
}

/// One instrument in the mix: a patch, and how loud it sits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// How notes refer to this track.
    pub name: String,
    pub patch: PatchRef,
    /// Linear gain applied to every note this track plays. The master limiter
    /// guarantees the sum never clips, so this is a balance control, not a safety one.
    #[serde(default = "one")]
    pub gain: f32,
}

/// A track's instrument: a path to a saved patch `.json`, or the patch inline.
///
/// The same duality the rest of the layer has (`Sound.Bake` takes a table,
/// `Sound.BakeJson` a string): a song being iterated on carries its patches inline
/// and stays one self-contained file, while a song built from a settled instrument
/// library points at it and stays short.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PatchRef {
    /// A path to a patch `.json` on disk.
    Path(String),
    /// The patch document itself. Boxed because it dwarfs the path variant.
    Inline(Box<Patch>),
}

/// A named block of notes, `beats` long. Patterns are just N beats — no time
/// signature, because nothing in the renderer needs bars.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// How long this block occupies in the arrangement, in beats. Notes may ring out
    /// past it (the next pattern starts on time regardless); this is the *slot*.
    pub beats: f32,
    pub notes: Vec<Note>,
}

/// One note: which track plays it, what pitch, when, for how long, how hard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// The [`Track::name`] that plays it.
    pub track: String,
    pub note: Pitch,
    /// Onset in beats from the start of its pattern.
    pub start: f32,
    /// Gate length in beats. The patch's release rings out after it.
    pub dur: f32,
    /// Velocity in `0..=1`, scaling this note's peak amplitude.
    #[serde(default = "one")]
    pub vel: f32,
}

/// A pitch, written the way a score does or as a raw MIDI number.
///
/// Untagged, and `Name` first: a JSON string can only be a name and a JSON number can
/// only be a MIDI value, so the two never race.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Pitch {
    /// Scientific pitch notation — `"C#4"`, `"Bb3"` (see
    /// [`parse_note`](crate::soundgen::parse_note)).
    Name(String),
    /// A MIDI note number; fractional values are legal microtonal pitches.
    Midi(f32),
}

impl Pitch {
    /// Resolve to a MIDI number, reporting an unparseable name verbatim.
    pub fn to_midi(&self) -> Result<f32, String> {
        match self {
            Pitch::Name(name) => super::note::parse_note(name),
            Pitch::Midi(midi) => Ok(*midi),
        }
    }
}

impl Song {
    /// Serialize to canonical JSON (pretty, with patterns in name order).
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Parse a song from its JSON form.
    pub fn from_json(s: &str) -> Result<Self, String> {
        serde_json::from_str(s).map_err(|e| e.to_string())
    }

    /// Total arrangement length in beats — the sum of its patterns' slots. Notes are
    /// allowed to ring out past this; it is where the *last pattern* ends, not where
    /// the audio does.
    pub fn arrangement_beats(&self) -> f32 {
        self.arrangement
            .iter()
            .filter_map(|name| self.patterns.get(name))
            .map(|p| p.beats)
            .sum()
    }

    /// Seconds per beat.
    pub fn beat_seconds(&self) -> f32 {
        60.0 / self.bpm
    }

    /// Reject a song the renderer can't honour, *before* any samples are produced —
    /// so a typo'd track or pattern name is a clear message rather than silence in
    /// the mix, which is the failure mode that would cost an agent a whole iteration
    /// to even notice.
    pub fn validate(&self) -> Result<(), String> {
        if !(self.bpm.is_finite() && self.bpm > 0.0) {
            return Err(format!("song bpm must be positive, got {}", self.bpm));
        }
        if self.tracks.is_empty() {
            return Err("song has no tracks".to_string());
        }
        if self.arrangement.is_empty() {
            return Err("song arrangement is empty — nothing would be rendered".to_string());
        }
        for name in &self.arrangement {
            if !self.patterns.contains_key(name) {
                return Err(format!(
                    "arrangement names pattern `{name}`, which is not defined"
                ));
            }
        }
        for (name, pattern) in &self.patterns {
            pattern.validate(name, &self.tracks)?;
        }
        Ok(())
    }
}

impl Pattern {
    /// Check one pattern's slot length and every note in it.
    fn validate(&self, name: &str, tracks: &[Track]) -> Result<(), String> {
        if !(self.beats.is_finite() && self.beats > 0.0) {
            return Err(format!(
                "pattern `{name}`: beats must be positive, got {}",
                self.beats
            ));
        }
        for (i, note) in self.notes.iter().enumerate() {
            if !tracks.iter().any(|t| t.name == note.track) {
                return Err(format!(
                    "pattern `{name}` note {i}: no track named `{}`",
                    note.track
                ));
            }
            if !(note.start.is_finite() && note.start >= 0.0) {
                return Err(format!(
                    "pattern `{name}` note {i}: start must be >= 0, got {}",
                    note.start
                ));
            }
            if !(note.dur.is_finite() && note.dur > 0.0) {
                return Err(format!(
                    "pattern `{name}` note {i}: dur must be positive, got {}",
                    note.dur
                ));
            }
            note.note.to_midi()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
