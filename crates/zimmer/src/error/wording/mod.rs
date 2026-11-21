//! What each refusal says out loud.

mod automation;
mod chain;
mod length;
mod lfo;
mod note;
mod pulse;
mod song;
mod source;
mod timing;

use std::fmt;

use super::SynthError;

impl SynthError {
    // Exhaustive, so a refusal with no wording fails to compile. Each arm hands a group to the
    // `say_*` that words exactly those variants; a table, as the enum is.
    #[rustfmt::skip]
    pub(super) fn say(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadCutoff { .. } | Self::TooManyEqBands { .. } | Self::UnknownSidechain { .. }
            | Self::SelfSidechain { .. } | Self::MisplacedSidechain { .. } => self.say_chain(f),
            Self::EmptyOscStack | Self::TooManyOscs { .. } | Self::SilentOscStack
            | Self::BadVoiceCount { .. } | Self::BadFmRatio { .. } | Self::EmptyPartials
            | Self::TooManyPartials { .. } | Self::SilentPartials | Self::BadPartialRatio { .. }
            | Self::BadOperatorRatio { .. } | Self::SilentCarriers { .. } => self.say_source(f),
            Self::TooManyLayers { .. } | Self::SilentLayer | Self::InLayer { .. }
            => self.say_layers(f),
            Self::LfoTwoRates { .. } | Self::LfoNoRate | Self::BadLfoBeats { .. }
            | Self::LfoBeatsWithoutTempo { .. } | Self::BadLfoOnset { .. }
            | Self::NegativeLfoRate { .. } => self.say_lfo(f),
            Self::PulseWidthOffSquare { .. } | Self::NoPulseToModulate => self.say_pulse(f),
            Self::BadSwing { .. } | Self::BadHumanize { .. } | Self::BadTrackFeel { .. }
            | Self::MisplacedBeats { .. } | Self::DelayTwoTimes { .. } => self.say_timing(f),
            Self::UnknownAutomationTrack { .. } | Self::DuplicateAutomation { .. }
            | Self::BadAutomationCurve { .. } | Self::BadAutomationPoint { .. }
            | Self::BadAutomationCutoff { .. } | Self::AutomationWithoutFilter { .. }
            | Self::AutomationWithoutRoom { .. } => self.say_automation(f),
            Self::BadDuration { .. } | Self::NoteTooLong { .. } | Self::BadFitSeconds { .. }
            | Self::FitLength { .. } | Self::BadAnchor { .. } => self.say_length(f),
            Self::BadFade { .. } | Self::StretchTooFar { .. } | Self::WrapWith { .. }
            | Self::WrapOverhang { .. } | Self::SongTooLong { .. } => self.say_ending(f),
            Self::BadBpm { .. } | Self::NoTracks | Self::EmptyArrangement | Self::NoLayers
            | Self::UnknownPattern { .. } | Self::UnknownTrackFilter { .. }
            | Self::UnknownSoloTrack { .. } | Self::BadTempoChange { .. }
            | Self::TempoOutOfOrder { .. } => self.say_song(f),
            Self::BadTranspose { .. } | Self::BadVelocityScale { .. } | Self::BadPatternBeats { .. }
            | Self::DiatonicWithoutKey { .. } | Self::TwoTransposes { .. }
            => self.say_arrangement(f),
            Self::UnknownTrack { .. } | Self::BadNoteStart { .. } | Self::BadNoteDuration { .. }
            | Self::UnresolvedPatch { .. } => self.say_pattern(f),
            Self::UnknownChord { .. } | Self::ChordOutOfRange { .. }
            | Self::SpelledChordOctave { .. } | Self::EmptyChord { .. } | Self::BadArp { .. }
            | Self::BadVoicing { .. } | Self::BadKey { .. } | Self::BadDegree { .. }
            | Self::DegreeWithoutKey { .. } | Self::DegreeOutOfRange { .. } => self.say_harmony(f),
            Self::BadStep { .. } | Self::StepsDoNotFit { .. } | Self::BadStepDiv { .. }
            | Self::AccentWithoutHeadroom { .. } | Self::TwiceAccented { .. }
            | Self::TwiceGhosted { .. } => self.say_steps(f),
            Self::EmptyNoteName | Self::BadNoteLetter { .. } | Self::BadOctave { .. }
            | Self::NoteOutOfRange { .. } => self.say_note(f),
        }
    }
}

impl std::error::Error for SynthError {}

impl fmt::Display for SynthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.say(f)
    }
}
