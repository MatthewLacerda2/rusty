//! Where a song writes the note a refusal is about, for the one refusal found only once the note is
//! played ([`SynthError::NoteTooLong`]).

use super::SynthError;

#[derive(Debug, Clone, PartialEq)]
pub struct NoteAt {
    pub pattern: String,
    pub track: String,
    pub start: f32,
}

impl SynthError {
    pub(crate) fn at(self, note: NoteAt) -> Self {
        match self {
            Self::NoteTooLong {
                at: None,
                held,
                tail,
                limit,
            } => Self::NoteTooLong {
                at: Some(note),
                held,
                tail,
                limit,
            },
            other => other,
        }
    }
}
