//! A [`Song`], written out as a Standard MIDI File.
//!
//! The other half of [`import`](super::import), and the reason it matters is
//! who is on the other end: a `.mid` opens in every DAW there is, so this is
//! how someone checks or finishes zimmer's work with the tools they already
//! know. [`export`] takes the document and hands back the file's bytes and a
//! list of what the file could not carry.
//!
//! **It writes what is played, not what is written.** A MIDI file has no
//! patterns, no chords, no degrees and no arrangement — it is notes in time —
//! so the song is played through first, the way the renderer plays it, and
//! the notes that would sound are the notes that are written:
//!
//! - **The arrangement, once, in order**, with each entry's transposes, its
//!   `vel_scale` and its muted tracks applied. A `loop` or `stretch` fit is not:
//!   the file is the piece as composed, and fitting it to a picture is the
//!   video's business, not the DAW's.
//! - **Chords, step strings and degrees become their notes**, through the same
//!   expansion the renderer uses, so the file cannot disagree with the bake
//!   about what a chord symbol meant.
//! - **Swing and articulations are applied** — they are where and how hard a
//!   note is played, and they are exact: an accent strikes harder, staccato
//!   and ghost notes are held shorter. A ghost's few milliseconds of
//!   earliness are not written, because they are seconds rather than beats.
//! - **Humanize is not.** It is a scatter drawn per render, and writing one
//!   draw of it into the file would bake an accident in as though it were the
//!   part; every DAW has a humanize of its own to apply on purpose.
//!
//! **One MIDI track per song track**, in the song's order and named for it,
//! after a conductor track holding the tempo map and the key signature. That
//! is format 1, which is what a DAW expects and what [`import`](super::import)
//! reads back as one song track each. Every track has a channel of its own —
//! the fifteen that are not channel 10, reused in order past fifteen tracks —
//! and **no program change**: a program number would pick a General MIDI
//! sound, and the song's sounds are patches this file has no way to describe.
//!
//! **Drums are the caller's to name** ([`Drum`]), because nothing in a song
//! says which track is one: a kick is a patch like any other, played at
//! whatever pitch its author tuned it to. A named track goes on channel 10,
//! where General MIDI reads a key as a drum rather than a pitch — its notes
//! keep their key numbers, or all become the one key given (`kick=36`), which
//! is what makes a separately written kick land on a DAW kit's kick.
//!
//! **Tempo ramps become steps** — MIDI has only jumps. The `tempo` module has
//! how fine, and why the steps land exactly where the ramp does.
//!
//! **Everything else is sound, and is not exported**: patches, effects, gain,
//! pan, automation, fades. The file is the score. What *is* lost from the
//! notes — a microtonal pitch, a pitch off the keyboard, a glide — is counted
//! in [`Exported::left_out`] rather than dropped silently, the way an import
//! names what it left behind.
//!
//! **No I/O**, for the reason the importer has none: this is arithmetic on a
//! document, and the caller decides where the bytes go.

mod drums;
mod play;
mod tempo;
mod write;

use std::fmt;

use crate::{Song, SynthError};

pub use drums::Drum;

/// The resolution every exported file is written at, in ticks per beat.
///
/// 1920 because it divides evenly by every resolution a file is commonly
/// written at — 96, 120, 192, 240, 384 (LilyPond's, and so the Mutopia
/// Project's), 480 and 960 — so a song imported from any of them lands on the
/// same ticks it came from, and a triplet at every level down to the 32nd is a
/// whole number of ticks. At 120 bpm one tick is a quarter of a millisecond.
pub const TICKS_PER_BEAT: u16 = 1920;

/// A song as MIDI, and what the file could not say.
#[derive(Debug, Clone, PartialEq)]
pub struct Exported {
    /// The Standard MIDI File, format 1.
    pub bytes: Vec<u8>,
    /// How many song tracks were written, one MIDI track each.
    pub tracks: usize,
    /// How many notes were written, across all of them.
    pub notes: usize,
    /// How many tempo events the conductor track holds, the opening one
    /// included — more than the song's map when a ramp was written as steps.
    pub tempo_events: usize,
    /// One sentence for each kind of thing the song played and the file does
    /// not — `"3 glides are written as plain notes …"`. Empty when every note
    /// went across exactly.
    pub left_out: Vec<String>,
}

/// Writes `song` as a Standard MIDI File, with `drums` naming the tracks to
/// put on the percussion channel.
///
/// Refuses a song that would not render, a drum naming no track, and a piece
/// too long for a MIDI file to time — each of which has no file to write.
pub fn export(song: &Song, drums: &[Drum]) -> Result<Exported, ExportError> {
    song.validate().map_err(ExportError::Song)?;
    let percussion = drums::resolve(song, drums)?;
    let played = play::parts(song, &percussion).map_err(ExportError::Song)?;
    let tempos = tempo::events(song);
    let mut left_out = played.unsaid.sentences();
    left_out.extend(tempos.sentences());
    let (bytes, conductor) = write::file(song, &played.parts, &tempos.events)?;
    left_out.extend(conductor);
    Ok(Exported {
        bytes,
        tracks: played.parts.len(),
        notes: played.parts.iter().map(|part| part.notes.len()).sum(),
        tempo_events: tempos.events.len(),
        left_out,
    })
}

/// Why a song could not be written as MIDI.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    /// The song itself does not render: a pattern that does not exist, a note
    /// on a track that does not, a key that is not one.
    Song(SynthError),
    /// A drum named a track the song does not have.
    NoSuchTrack {
        /// The name given.
        track: String,
    },
    /// A drum key past 127, which no MIDI note is.
    DrumKey {
        /// The track it was given for.
        track: String,
        /// The key given.
        key: u8,
    },
    /// Two events further apart than a MIDI file can time: 2²⁸ ticks, about
    /// 140 thousand beats.
    TooLong,
    /// The encoder refused what it was handed — past 65,535 tracks.
    Unwritable {
        /// What it said.
        why: String,
    },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Song(error) => write!(f, "the song does not render: {error}"),
            Self::NoSuchTrack { track } => write!(
                f,
                "`{track}` is named as a drum track, and the song has no track by that name"
            ),
            Self::DrumKey { track, key } => write!(
                f,
                "`{track}={key}`: a MIDI key is 0 to 127 — 36 is General MIDI's kick, 38 \
                 its snare, 42 its closed hi-hat"
            ),
            Self::TooLong => f.write_str("this song is too long for a MIDI file to time"),
            Self::Unwritable { why } => write!(f, "the MIDI file could not be written: {why}"),
        }
    }
}

impl std::error::Error for ExportError {}
