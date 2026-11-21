//! The song document: patterns, arrangement, tracks.

pub(crate) mod anchor;
pub(crate) mod arrangement;
pub(crate) mod automate;
mod cap;
pub(crate) mod chord;
pub(crate) mod clock;
pub(crate) mod excerpt;
mod forms;
pub(crate) mod key;
pub(crate) mod play;
pub(crate) mod render;
pub(crate) mod sections;
pub(crate) mod shape;
pub(crate) mod tempo;
pub(crate) mod timing;
mod validate;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::SynthError;
use crate::patch::{Fx, Patch};

pub use anchor::Anchor;
pub use arrangement::{ArrangementEntry, Layer, Layers, Play};
pub use automate::{Automation, Easing, Param, Point};
pub use cap::MAX_SONG_SECONDS;
pub use chord::{Arp, Chord, Leading, Voicing};
pub use excerpt::{Excerpt, Span, SpanError, Unit, Window};
pub use key::{Degree, DegreeNote, Key, Mode};
pub use play::articulation::Articulation;
pub use play::feel::Humanize;
pub use play::steps::Steps;
pub use render::{render_excerpt, render_song, InlineOnly, PatchResolver};
pub use tempo::TempoChange;
pub use timing::{Fade, Fit, FitMode, FitTo, Tail};

fn one() -> f32 {
    1.0
}

fn no_swing(swing: &f32) -> bool {
    *swing == 0.0
}

fn centred(pan: &f32) -> bool {
    *pan == 0.0
}

fn fully_sent(send: &f32) -> bool {
    *send == 1.0
}

fn on_the_beat(push: &f32) -> bool {
    *push == 0.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Song {
    pub bpm: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tempo: Vec<TempoChange>,
    #[serde(default)]
    pub seed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub tracks: Vec<Track>,
    pub patterns: BTreeMap<String, Pattern>,
    pub arrangement: Vec<ArrangementEntry>,
    #[serde(default, skip_serializing_if = "no_swing")]
    pub swing: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humanize: Option<Humanize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx: Vec<Fx>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub automation: Vec<Automation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<Fit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<Anchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fade: Option<Fade>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<Tail>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub name: String,
    pub patch: PatchRef,
    #[serde(default = "one")]
    pub gain: f32,
    #[serde(default, skip_serializing_if = "centred")]
    pub pan: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fx: Vec<Fx>,
    #[serde(default = "one", skip_serializing_if = "fully_sent")]
    pub send: f32,
    #[serde(default, skip_serializing_if = "on_the_beat")]
    pub push: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swing: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humanize: Option<Humanize>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum PatchRef {
    Named(String),
    Inline(Box<Patch>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    pub beats: f32,
    pub notes: Vec<PatternEntry>,
}

impl Pattern {
    pub(crate) fn voices(&self, key: Option<&Key>) -> Result<Vec<Note>, SynthError> {
        let around = Context {
            beats: self.beats,
            key,
        };
        let mut voiced = Vec::with_capacity(self.notes.len());
        let mut led = chord::Led::default();
        for entry in &self.notes {
            entry.voice_into(around, &mut led, &mut voiced)?;
        }
        Ok(voiced)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Context<'a> {
    pub(crate) beats: f32,
    pub(crate) key: Option<&'a Key>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum PatternEntry {
    Note(Note),
    Degree(DegreeNote),
    Chord(Chord),
    Steps(Steps),
}

impl PatternEntry {
    pub fn track(&self) -> &str {
        match self {
            Self::Note(note) => &note.track,
            Self::Degree(degree) => &degree.track,
            Self::Chord(chord) => &chord.track,
            Self::Steps(steps) => &steps.track,
        }
    }

    pub fn start(&self) -> f32 {
        match self {
            Self::Note(note) => note.start,
            Self::Degree(degree) => degree.start,
            Self::Chord(chord) => chord.start,
            Self::Steps(steps) => steps.start,
        }
    }

    pub fn dur(&self) -> f32 {
        match self {
            Self::Note(note) => note.dur,
            Self::Degree(degree) => degree.dur,
            Self::Chord(chord) => chord.dur,
            Self::Steps(steps) => steps.gate(),
        }
    }

    pub(crate) fn voice_into(
        &self,
        around: Context<'_>,
        led: &mut chord::Led,
        out: &mut Vec<Note>,
    ) -> Result<(), SynthError> {
        match self {
            Self::Note(note) => {
                out.push(note.clone());
                Ok(())
            }
            Self::Degree(degree) => degree.voice_into(around.key, out),
            Self::Chord(chord) => chord.voice_into(led, out),
            Self::Steps(steps) => steps.voice_into(around.beats, out),
        }
    }
}

impl From<Note> for PatternEntry {
    fn from(note: Note) -> Self {
        Self::Note(note)
    }
}

impl From<DegreeNote> for PatternEntry {
    fn from(degree: DegreeNote) -> Self {
        Self::Degree(degree)
    }
}

impl From<Chord> for PatternEntry {
    fn from(chord: Chord) -> Self {
        Self::Chord(chord)
    }
}

impl From<Steps> for PatternEntry {
    fn from(steps: Steps) -> Self {
        Self::Steps(steps)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub track: String,
    pub note: Pitch,
    pub start: f32,
    pub dur: f32,
    #[serde(default = "one")]
    pub vel: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Pitch {
    Name(String),
    Midi(f32),
}

impl Pitch {
    pub fn to_midi(&self) -> Result<f32, SynthError> {
        match self {
            Pitch::Name(name) => crate::note::parse_note(name),
            Pitch::Midi(midi) => Ok(*midi),
        }
    }
}

impl Song {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn arrangement_beats(&self) -> f32 {
        self.arrangement
            .iter()
            .filter_map(|entry| self.slot_beats(entry))
            .sum()
    }

    pub(crate) fn slot_beats(&self, entry: &ArrangementEntry) -> Option<f32> {
        entry
            .layers()
            .iter()
            .filter_map(|layer| self.patterns.get(layer.pattern()))
            .map(|pattern| pattern.beats)
            .reduce(f32::max)
    }

    pub fn beat_seconds(&self) -> f32 {
        60.0 / self.bpm
    }

    pub fn tail(&self) -> Tail {
        self.tail.unwrap_or_default()
    }

    pub fn key(&self) -> Result<Option<Key>, SynthError> {
        self.key.as_deref().map(Key::parse).transpose()
    }

    pub fn humanize(&self) -> Humanize {
        self.humanize.unwrap_or_default()
    }
}
