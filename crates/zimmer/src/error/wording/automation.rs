//! What an automation curve is refused with: a curve that names nothing, says nothing, or moves
//! nothing anyone can hear.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    pub(super) fn say_automation(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownAutomationTrack { track, param, .. } => write!(
                f,
                "song: automation of `{param}` names `{track}`, which is not a track here"
            ),
            Self::DuplicateAutomation { track, param, .. } => write!(
                f,
                "song: track `{track}` has two curves for `{param}` — one parameter moves one way"
            ),
            Self::BadAutomationCurve {
                track, param, why, ..
            } => write!(f, "song: automation of `{param}` on track `{track}`: {why}"),
            Self::BadAutomationPoint {
                track,
                param,
                field,
                value,
                ..
            } => write!(
                f,
                "song: automation of `{param}` on `{track}`: bad `{field}`, got {value}"
            ),
            Self::BadAutomationCutoff { track, cutoff, .. } => write!(
                f,
                "song: automation of `cutoff` on `{track}`: must be positive Hz, got {cutoff}"
            ),
            Self::AutomationWithoutFilter { track, .. } => write!(
                f,
                "song: automation of `cutoff` on track `{track}`, whose patch has no filter"
            ),
            Self::AutomationWithoutRoom { track, .. } => write!(
                f,
                "song: automation of `send` on track `{track}`, but the song's `fx` has no \
                 `reverb` or `delay` for a send to reach"
            ),
            other => write!(f, "{other:?}"),
        }
    }
}
