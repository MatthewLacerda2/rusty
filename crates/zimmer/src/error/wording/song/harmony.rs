//! What chords, arps, voicings, keys and scale degrees are refused with.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(in crate::error) fn say_harmony(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownChord { chord, .. } => write!(
                f,
                "song: `{chord}` is not a chord name — write the pitches out instead, as `[\"D3\", \"F3\", \"A3\"]`"
            ),
            Self::ChordOutOfRange {
                chord, oct, midi, ..
            } => write!(
                f,
                "song: chord `{chord}` at octave {oct} reaches MIDI {midi}, outside 0..=127"
            ),
            Self::SpelledChordOctave { track, start, .. } => write!(
                f,
                "song: track `{track}` at beat {start}: `oct` means nothing beside spelled pitches"
            ),
            Self::EmptyChord { track, start, .. } => write!(
                f,
                "song: track `{track}` at beat {start}: a chord needs at least one pitch"
            ),
            Self::BadArp { track, start, why } | Self::BadVoicing { track, start, why } => {
                write!(f, "song: track `{track}` at beat {start}: {why}")
            }
            Self::BadKey { key, .. } => write!(
                f,
                "song: `{key}` is not a key — write a tonic and a mode, like `D minor`, `F# lydian` or `Bb major`"
            ),
            Self::BadDegree { degree, .. } => write!(
                f,
                "song: `{degree}` is not a scale degree — they count from 1, with accidentals in front (`b3`, `#4`)"
            ),
            Self::DegreeWithoutKey { track, start, .. } => write!(
                f,
                "song: track `{track}` at beat {start}: a `degree` needs the song to declare a `key`"
            ),
            Self::DegreeOutOfRange {
                degree, oct, midi, ..
            } => write!(
                f,
                "song: degree `{degree}` with the tonic at octave {oct} reaches MIDI {midi}, outside 0..=127"
            ),
            other => other.say_steps(f),
        }
    }
}
