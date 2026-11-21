//! What a note name is refused with.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_note(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyNoteName => write!(f, "empty note name"),
            Self::BadNoteLetter { name, letter, .. } => {
                write!(f, "note `{name}`: expected a letter A–G, got `{letter}`")
            }
            Self::BadOctave { name, octave, .. } => {
                write!(f, "note `{name}`: `{octave}` is not an octave number")
            }
            Self::NoteOutOfRange { name, midi, .. } => {
                write!(f, "note `{name}`: MIDI {midi} is outside 0..=127")
            }
            _ => unreachable!("`say_note` is handed only the refusals it words"),
        }
    }
}
