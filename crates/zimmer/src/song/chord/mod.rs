//! A chord: one entry in a pattern that sounds as several notes.

pub(crate) mod arp;
pub(crate) mod lead;
pub(crate) mod name;

use serde::{Deserialize, Serialize};

pub use arp::Arp;
pub use lead::Leading;
pub(crate) use lead::Led;

use super::{Articulation, Note, Pitch, one};
use crate::error::SynthError;
use crate::note::MIDI_RANGE;

const DEFAULT_OCT: i32 = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chord {
    pub track: String,
    pub chord: Voicing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oct: Option<i32>,
    pub start: f32,
    pub dur: f32,
    #[serde(default = "one")]
    pub vel: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arp: Option<Arp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub div: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inv: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voicing: Option<Leading>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Voicing {
    Name(String),
    Pitches(Vec<Pitch>),
}

impl Chord {
    pub(crate) fn voice_into(&self, led: &mut Led, out: &mut Vec<Note>) -> Result<(), SynthError> {
        let voices = match &self.chord {
            Voicing::Name(name) => self.named(name, led)?,
            Voicing::Pitches(pitches) => self.spelled(pitches)?,
        };
        // Here rather than beside the spelling that can produce one, because an arpeggio walks
        // whatever this is and "no voices" has to be a refusal before anything walks it.
        if voices.is_empty() {
            return Err(SynthError::EmptyChord {
                track: self.track.clone(),
                start: self.start,
            });
        }
        match self.arp {
            Some(arp) => arp::spread(self, arp, &voices, out)?,
            None => {
                self.check_no_arp_fields()?;
                out.extend(voices.iter().cloned());
            }
        }
        led.record(&self.track, &voices);
        Ok(())
    }

    fn bad_voicing(&self, why: &'static str) -> SynthError {
        SynthError::BadVoicing {
            track: self.track.clone(),
            start: self.start,
            why,
        }
    }

    fn check_no_arp_fields(&self) -> Result<(), SynthError> {
        let refuse = |why| {
            Err(SynthError::BadArp {
                track: self.track.clone(),
                start: self.start,
                why,
            })
        };
        if self.div.is_some() {
            return refuse("`div` is the step of a figure, so it means nothing without an `arp`");
        }
        if self.gate.is_some() {
            return refuse(
                "`gate` is the length of one note of a figure, so it means nothing without an `arp` — a block chord's gate is its `dur`",
            );
        }
        Ok(())
    }

    fn named(&self, name: &str, led: &Led) -> Result<Vec<Note>, SynthError> {
        let oct = self.oct.unwrap_or(DEFAULT_OCT);
        let spelled = name::spell(name)?;
        let root = (oct + 1) * 12 + spelled.root;
        let range = MIDI_RANGE.0 as i32..=MIDI_RANGE.1 as i32;
        let on_keys = |midis: &[i32]| match midis.iter().find(|midi| !range.contains(midi)) {
            Some(&midi) => Err(SynthError::ChordOutOfRange {
                chord: name.to_owned(),
                oct,
                midi,
            }),
            None => Ok(()),
        };
        let (bass, upper): (Vec<i32>, Vec<i32>) = spelled
            .offsets
            .iter()
            .map(|offset| root + offset)
            .partition(|&midi| midi < root);
        let stack = lead::Stack {
            bass: bass.first().copied(),
            upper,
        };
        on_keys(&stack.turned(0))?;
        let midis = match (self.inv, self.voicing) {
            (Some(_), Some(_)) => {
                return Err(self.bad_voicing(
                    "`inv` and `voicing` both choose the inversion — write one of them",
                ));
            }
            (Some(inv), None) => {
                if usize::from(inv) >= stack.upper.len() {
                    return Err(self.bad_voicing(
                        "`inv` is past this chord's last inversion, which is one fewer than its voices above any slash bass — the next is root position an octave up, which is a higher `oct`",
                    ));
                }
                stack.turned(i32::from(inv))
            }
            (None, Some(Leading::Near)) => match led.last(&self.track) {
                Some(previous) => stack.nearest(previous, &range),
                None => stack.turned(0),
            },
            (None, None) => stack.turned(0),
        };
        on_keys(&midis)?;
        Ok(midis
            .into_iter()
            .map(|midi| self.voice(Pitch::Midi(midi as f32)))
            .collect())
    }

    fn spelled(&self, pitches: &[Pitch]) -> Result<Vec<Note>, SynthError> {
        if let Some(oct) = self.oct {
            return Err(SynthError::SpelledChordOctave {
                track: self.track.clone(),
                start: self.start,
                oct,
            });
        }
        if self.inv.is_some() || self.voicing.is_some() {
            return Err(self.bad_voicing(
                "`inv` and `voicing` choose how a named chord is voiced, and spelled pitches are already a voicing",
            ));
        }
        pitches
            .iter()
            // Resolved here so a pitch that does not parse is reported against the chord holding it
            // rather than at the sample loop.
            .map(|pitch| pitch.to_midi().map(|_| self.voice(pitch.clone())))
            .collect()
    }

    fn voice(&self, note: Pitch) -> Note {
        Note {
            track: self.track.clone(),
            note,
            start: self.start,
            dur: self.dur,
            vel: self.vel,
            articulation: self.articulation,
        }
    }
}

#[cfg(test)]
mod tests;
