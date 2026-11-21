//! Where a song writes the note a refusal is about, for the one refusal found
//! only once the note is played ([`SynthError::NoteTooLong`]).

use super::SynthError;

/// Where a song writes a note, for a refusal that is only found once the
/// note is played: the pattern, the track and where in the pattern it starts.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteAt {
    /// The pattern holding the note.
    pub pattern: String,
    /// The track it is played on.
    pub track: String,
    /// Where in the pattern it starts, in beats.
    pub start: f32,
}

impl SynthError {
    /// This refusal, told where the song writes the note it is about — for
    /// the one refusal of a note found only once it is played. Every other
    /// refusal is handed back as it was.
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
