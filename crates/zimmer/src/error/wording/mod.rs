//! What each refusal says out loud.

mod automation;
mod length;
mod lfo;
mod pulse;
mod source;
mod timing;

use std::fmt;

use super::SynthError;

impl SynthError {
    pub(super) fn say(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOscStack
            | Self::TooManyOscs { .. }
            | Self::SilentOscStack
            | Self::BadVoiceCount { .. }
            | Self::BadFmRatio { .. }
            | Self::EmptyPartials
            | Self::TooManyPartials { .. }
            | Self::SilentPartials
            | Self::BadPartialRatio { .. }
            | Self::BadOperatorRatio { .. }
            | Self::SilentCarriers { .. }
            | Self::TooManyLayers { .. }
            | Self::SilentLayer
            | Self::InLayer { .. } => self.say_source(f),
            Self::BadCutoff { cutoff, .. } => write!(
                f,
                "patch: filter `cutoff` must be positive Hz, got {cutoff}"
            ),
            Self::TooManyEqBands { found, limit, .. } => {
                write!(f, "fx: `eq` takes at most {limit} bands, got {found}")
            }
            Self::UnknownSidechain { track, key, .. } => write!(
                f,
                "song: track `{track}` is keyed from `{key}`, which is not a track in this song"
            ),
            Self::SelfSidechain { track, .. } => write!(
                f,
                "song: track `{track}` is keyed from itself — leave `sidechain` out for a compressor that listens to its own part"
            ),
            Self::MisplacedSidechain { place, key, .. } => write!(
                f,
                "{place}: `compress` is keyed from `{key}`, and only a track's own chain sits where one track can listen to another"
            ),
            Self::NegativeLfoRate { rate, .. } => {
                write!(f, "patch: lfo `rate` must not be negative, got {rate}")
            }
            Self::LfoTwoRates { .. }
            | Self::LfoNoRate
            | Self::BadLfoBeats { .. }
            | Self::LfoBeatsWithoutTempo { .. }
            | Self::BadLfoOnset { .. } => self.say_lfo(f),
            Self::PulseWidthOffSquare { .. } | Self::NoPulseToModulate => self.say_pulse(f),

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

            Self::BadBpm { bpm, .. } => write!(f, "song: bpm must be positive, got {bpm}"),
            Self::NoTracks => write!(f, "song: no tracks"),
            Self::EmptyArrangement => {
                write!(f, "song: arrangement is empty — nothing would be rendered")
            }
            Self::NoLayers => write!(
                f,
                "song: arrangement entry has an empty `layers` — name at least one pattern, or write a rest as a pattern with no notes"
            ),
            Self::UnknownPattern { pattern, .. } => write!(
                f,
                "song: arrangement names pattern `{pattern}`, which is not defined"
            ),
            Self::UnknownTrackFilter { pattern, track, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: no track named `{track}` to play"
            ),
            Self::BadTranspose {
                pattern, transpose, ..
            } => write!(
                f,
                "song: arrangement entry for `{pattern}`: transpose must be finite, got {transpose}"
            ),
            Self::BadVelocityScale { pattern, scale, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: vel_scale must be >= 0, got {scale}"
            ),
            Self::BadPatternBeats { pattern, beats, .. } => write!(
                f,
                "song: pattern `{pattern}`: beats must be positive, got {beats}"
            ),
            Self::UnknownSoloTrack { track } => {
                write!(f, "song: no track named `{track}` to render on its own")
            }
            Self::UnknownTrack {
                pattern,
                index,
                track,
                ..
            } => write!(
                f,
                "song: pattern `{pattern}` note {index}: no track named `{track}`"
            ),
            Self::BadNoteStart {
                pattern,
                index,
                start,
                ..
            } => write!(
                f,
                "song: pattern `{pattern}` note {index}: start must be >= 0, got {start}"
            ),
            Self::BadNoteDuration {
                pattern,
                index,
                dur,
                ..
            } => write!(
                f,
                "song: pattern `{pattern}` note {index}: dur must be positive, got {dur}"
            ),
            Self::UnknownChord { chord, .. } => write!(
                f,
                "song: `{chord}` is not a chord name — see the chord table in the recipes guide (`guide recipes`, or docs/recipes.md), or write the pitches out as `[\"D3\", \"F3\", \"A3\"]`"
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
            Self::BadStep {
                track,
                character,
                step,
                ..
            } => write!(
                f,
                "song: track `{track}`: `{character}` at step {step} is not a step — use `x` (a hit), `X` (an accent), `o` (a ghost) or `-` (a rest), and nothing else"
            ),
            Self::StepsDoNotFit {
                track,
                start,
                div,
                beats,
                written,
                needed,
                ..
            } => write!(
                f,
                "song: track `{track}`: {written} steps of {div} beats from beat {start} do not fill the pattern's {beats} — {needed} would"
            ),
            Self::BadStepDiv {
                track,
                start,
                div,
                beats,
                ..
            } => write!(
                f,
                "song: track `{track}`: no whole number of {div}-beat steps fills the {beats} beats from beat {start}"
            ),
            Self::AccentWithoutHeadroom { track, vel, .. } => write!(
                f,
                "song: track `{track}`: `X` and `x` are the same hit at `vel` {vel} — write a `vel` below 1 for the plain hits to be softer than the accents"
            ),
            Self::TwiceAccented { track, .. } => write!(
                f,
                "song: track `{track}`: a step string cannot be played `accent` and also mark accents with `X` — accent the whole run, or mark the hits"
            ),
            Self::TwiceGhosted { track, .. } => write!(
                f,
                "song: track `{track}`: a step string cannot be played `ghost` and also mark ghosts with `o` — ghost the whole run, or mark the hits"
            ),
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
            Self::DiatonicWithoutKey { pattern, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: `transpose_degrees` needs the song to declare a `key` — or use `transpose` for a chromatic shift"
            ),
            Self::TwoTransposes { pattern, .. } => write!(
                f,
                "song: arrangement entry for `{pattern}`: `transpose` is chromatic and `transpose_degrees` moves within the key — write one or the other"
            ),
            Self::BadTempoChange { index, beat, bpm } => write!(
                f,
                "song: `tempo[{index}]` must be after beat 0 (the tempo there is `bpm`) and at a positive bpm, got beat {beat} at {bpm} bpm"
            ),
            Self::TempoOutOfOrder {
                index,
                beat,
                previous,
            } => write!(
                f,
                "song: `tempo[{index}]` is at beat {beat}, not after the change before it at beat {previous} — tempo changes must ascend"
            ),
            // Where notes and echoes land against the beat — swing, humanise, a track's own feel
            // and a delay in beats — in `timing`.
            Self::BadSwing { .. }
            | Self::BadHumanize { .. }
            | Self::BadTrackFeel { .. }
            | Self::MisplacedBeats { .. }
            | Self::DelayTwoTimes { .. } => self.say_timing(f),
            Self::UnknownAutomationTrack { .. }
            | Self::DuplicateAutomation { .. }
            | Self::BadAutomationCurve { .. }
            | Self::BadAutomationPoint { .. }
            | Self::BadAutomationCutoff { .. }
            | Self::AutomationWithoutFilter { .. }
            | Self::AutomationWithoutRoom { .. } => self.say_automation(f),
            Self::BadDuration { .. }
            | Self::NoteTooLong { .. }
            | Self::BadFitSeconds { .. }
            | Self::FitLength { .. }
            | Self::BadAnchor { .. }
            | Self::BadFade { .. }
            | Self::StretchTooFar { .. }
            | Self::WrapWith { .. }
            | Self::WrapOverhang { .. }
            | Self::SongTooLong { .. } => self.say_length(f),

            Self::UnresolvedPatch {
                track,
                reference,
                reason,
                ..
            } => write!(
                f,
                "song: track `{track}`: cannot resolve patch `{reference}`: {reason}"
            ),
        }
    }
}

impl std::error::Error for SynthError {}

impl fmt::Display for SynthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.say(f)
    }
}
