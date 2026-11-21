//! An entry in the running order: a pattern — or several at once — and optionally how to play each.

use serde::{Deserialize, Serialize};

use super::Key;
use crate::note::MIDI_RANGE;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ArrangementEntry {
    Single(Layer),
    Layered(Layers),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layers {
    pub layers: Vec<Layer>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Layer {
    Name(String),
    Transformed(Play),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Play {
    pub pattern: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transpose: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transpose_degrees: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vel_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<String>>,
}

impl ArrangementEntry {
    pub fn layers(&self) -> &[Layer] {
        match self {
            Self::Single(layer) => std::slice::from_ref(layer),
            Self::Layered(layered) => &layered.layers,
        }
    }

    pub fn label(&self) -> String {
        let names: Vec<&str> = self.layers().iter().map(Layer::pattern).collect();
        names.join(" + ")
    }
}

impl Layer {
    pub fn pattern(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Transformed(play) => &play.pattern,
        }
    }

    pub fn transpose(&self) -> f32 {
        match self {
            Self::Name(_) => 0.0,
            Self::Transformed(play) => play.transpose.unwrap_or(0.0),
        }
    }

    pub fn transpose_degrees(&self) -> i32 {
        match self {
            Self::Name(_) => 0,
            Self::Transformed(play) => play.transpose_degrees.unwrap_or(0),
        }
    }

    pub fn played_pitch(&self, midi: f32, key: Option<&Key>) -> f32 {
        let moved = match key {
            Some(key) => key.shift(midi, self.transpose_degrees()),
            None => midi,
        };
        (moved + self.transpose()).clamp(MIDI_RANGE.0, MIDI_RANGE.1)
    }

    pub fn vel_scale(&self) -> f32 {
        match self {
            Self::Name(_) => 1.0,
            Self::Transformed(play) => play.vel_scale.unwrap_or(1.0),
        }
    }

    pub fn plays(&self, track: &str) -> bool {
        match self {
            Self::Name(_) => true,
            Self::Transformed(play) => play
                .tracks
                .as_ref()
                .is_none_or(|only| only.iter().any(|name| name == track)),
        }
    }
}

impl From<&str> for Layer {
    fn from(name: &str) -> Self {
        Self::Name(name.to_owned())
    }
}

impl From<String> for Layer {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}

impl From<Play> for Layer {
    fn from(play: Play) -> Self {
        Self::Transformed(play)
    }
}

impl From<&str> for ArrangementEntry {
    fn from(name: &str) -> Self {
        Self::Single(name.into())
    }
}

impl From<String> for ArrangementEntry {
    fn from(name: String) -> Self {
        Self::Single(name.into())
    }
}

impl From<Play> for ArrangementEntry {
    fn from(play: Play) -> Self {
        Self::Single(play.into())
    }
}

impl From<Layer> for ArrangementEntry {
    fn from(layer: Layer) -> Self {
        Self::Single(layer)
    }
}

impl From<Layers> for ArrangementEntry {
    fn from(layered: Layers) -> Self {
        Self::Layered(layered)
    }
}
