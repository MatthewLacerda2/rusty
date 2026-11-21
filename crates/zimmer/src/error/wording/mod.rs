//! What each refusal says out loud.

mod automation;
mod length;
mod lfo;
mod pulse;
mod song;
mod source;
mod timing;

use std::fmt;

use super::SynthError;

impl SynthError {
    // Exhaustive, so a refusal with no wording fails to compile. A group's arm hands it to
    // the `say_*` that words exactly those variants.
    pub(super) fn say(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            | Self::SilentCarriers { .. } => self.say_source(f),
            Self::TooManyLayers { .. }
            | Self::SilentLayer
            | Self::InLayer { .. } => self.say_layers(f),
            Self::LfoTwoRates { .. }
            | Self::LfoNoRate
            | Self::BadLfoBeats { .. }
            | Self::LfoBeatsWithoutTempo { .. }
            | Self::BadLfoOnset { .. } => self.say_lfo(f),
            Self::PulseWidthOffSquare { .. }
            | Self::NoPulseToModulate => self.say_pulse(f),
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
            | Self::BadAnchor { .. } => self.say_length(f),
            Self::BadFade { .. }
            | Self::StretchTooFar { .. }
            | Self::WrapWith { .. }
            | Self::WrapOverhang { .. }
            | Self::SongTooLong { .. } => self.say_ending(f),
            Self::BadBpm { .. }
            | Self::NoTracks
            | Self::EmptyArrangement
            | Self::NoLayers
            | Self::UnknownPattern { .. }
            | Self::UnknownTrackFilter { .. }
            | Self::UnknownSoloTrack { .. }
            | Self::BadTempoChange { .. }
            | Self::TempoOutOfOrder { .. } => self.say_song(f),
            Self::BadTranspose { .. }
            | Self::BadVelocityScale { .. }
            | Self::BadPatternBeats { .. }
            | Self::DiatonicWithoutKey { .. }
            | Self::TwoTransposes { .. } => self.say_arrangement(f),
            Self::UnknownTrack { .. }
            | Self::BadNoteStart { .. }
            | Self::BadNoteDuration { .. }
            | Self::UnresolvedPatch { .. } => self.say_pattern(f),
            Self::UnknownChord { .. }
            | Self::ChordOutOfRange { .. }
            | Self::SpelledChordOctave { .. }
            | Self::EmptyChord { .. }
            | Self::BadArp { .. }
            | Self::BadVoicing { .. }
            | Self::BadKey { .. }
            | Self::BadDegree { .. }
            | Self::DegreeWithoutKey { .. }
            | Self::DegreeOutOfRange { .. } => self.say_harmony(f),
            Self::BadStep { .. }
            | Self::StepsDoNotFit { .. }
            | Self::BadStepDiv { .. }
            | Self::AccentWithoutHeadroom { .. }
            | Self::TwiceAccented { .. }
            | Self::TwiceGhosted { .. } => self.say_steps(f),
        }
    }
}

impl std::error::Error for SynthError {}

impl fmt::Display for SynthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.say(f)
    }
}
