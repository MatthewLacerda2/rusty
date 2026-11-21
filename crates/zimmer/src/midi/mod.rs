//! A Standard MIDI File, read into a [`Song`] — and a song written back out
//! as one, by [`export`], whose own module doc has what that direction keeps.
//!
//! The ground truth of almost any arrangement someone brings — a DAW export, a
//! keyboard take, a transcription off a sheet-music site — is a `.mid`, and
//! this is how one becomes a song recipe rather than an afternoon of scripting
//! outside scorsese. [`import`] takes the file's bytes and hands back the
//! document and a list of what it could not carry.
//!
//! **It maps structure and never guesses intent.** That is the whole design,
//! and each rule below is a case of it:
//!
//! - **One song track per MIDI track and channel** that plays a note. A piano
//!   part on one track stays one track; the importer does not split the left
//!   hand into a bass and the right into a melody, because which notes are
//!   the bass is a reading of the music, and the file did not write one.
//! - **Channel 10 is percussion**, because General MIDI reserves it: that
//!   part gets a drum voice, and its notes stay the key numbers the file wrote
//!   (`36`, `38`), because on that channel a number names a drum rather than a
//!   pitch. It is not split into a kick and a snare track either.
//! - **Patterns are a grid of bars**, [`BARS_PER_PATTERN`] to a pattern,
//!   counted from the file's own time signatures and named for the bars they
//!   hold (`bars-9-16`). No repeat is detected and no pattern is reused: the
//!   arrangement names every pattern once, in order. The `bars` module has
//!   why eight bars rather than one pattern for the whole piece.
//! - **The tempo map is the file's**: its first tempo becomes `bpm` and every
//!   later one a [jump](crate::song::TempoChange::jump), which is exactly what
//!   a MIDI tempo event is.
//! - **The key is the file's first key signature**, if it has one. Notes are
//!   written as names (`C#4`), never as degrees — a degree would claim the
//!   note belongs to the key, which is an analysis.
//! - **The sounds are placeholders.** Every pitched track plays one plain
//!   voice and every drum track one noise burst; a program change is reported,
//!   not emulated. See the `sound` module.
//!
//! What the song has no room for — the sustain pedal, pitch bends, volume and
//! pan controllers — is **counted and named** in [`Imported::left_out`] rather
//! than dropped silently, so an import that sounds drier than the file is
//! explained by the command that made it.
//!
//! **No I/O.** Parsing a byte slice is arithmetic, which is why this lives
//! beside the document it produces rather than in a crate that opens files:
//! the caller reads the file, and this never learns where it came from. The
//! parser underneath is `midly`, and nothing of it leaks out of this module —
//! its types stay inside, and [`MidiError`] says what went wrong in words.

mod bars;
mod build;
mod export;
mod read;
mod sound;

use std::fmt;

use crate::Song;

pub use bars::BARS_PER_PATTERN;
pub use export::{Drum, ExportError, Exported, TICKS_PER_BEAT, export};

/// A MIDI file read as a song, and what the song could not say.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// The recipe: every note of the file, on plain default instruments.
    pub song: Song,
    /// One sentence for each kind of thing the file carried and the song does
    /// not — `"412 controller changes (sustain pedal, …) are not imported"`.
    /// Empty when nothing was left behind.
    pub left_out: Vec<String>,
}

/// Reads a Standard MIDI File's bytes as a [`Song`].
///
/// Refuses a file that is not MIDI, one timed in SMPTE frames rather than
/// beats, a format-2 file of independent sequences, and one with no notes —
/// each of which has no honest reading as one piece of music in beats.
pub fn import(bytes: &[u8]) -> Result<Imported, MidiError> {
    let smf = midly::Smf::parse(bytes).map_err(|error| MidiError::Unreadable {
        why: error.to_string(),
    })?;
    build::song(read::score(&smf)?)
}

/// Why a file could not be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidiError {
    /// Not a Standard MIDI File, or one too damaged to read.
    Unreadable {
        /// What the parser said.
        why: String,
    },
    /// Timed in SMPTE frames: its clock is seconds, and a song's is beats.
    Timecode,
    /// A header claiming zero ticks per beat, which cannot place any event.
    NoResolution,
    /// Format 2: a set of independent sequences rather than one piece.
    Sequential,
    /// Not one note anywhere in the file.
    NoNotes,
    /// So long that counting its bars would be an allocation nobody asked for.
    TooLong,
}

impl fmt::Display for MidiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { why } => write!(f, "not a readable MIDI file: {why}"),
            Self::Timecode => f.write_str(
                "this MIDI file is timed in SMPTE frames rather than beats, and a song is \
                 written in beats — export it from the sequencer with a tempo instead",
            ),
            Self::NoResolution => f.write_str("this MIDI file says a beat is zero ticks long"),
            Self::Sequential => f.write_str(
                "this is a format-2 MIDI file — independent sequences, not one piece — and \
                 there is no single song to read it as",
            ),
            Self::NoNotes => f.write_str("this MIDI file plays no notes"),
            Self::TooLong => f.write_str("this MIDI file is longer than ten thousand bars"),
        }
    }
}

impl std::error::Error for MidiError {}
