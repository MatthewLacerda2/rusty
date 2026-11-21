//! What an automation curve is refused with: a curve that names nothing, says
//! nothing, or moves nothing anyone can hear.
//!
//! Out of [`super`] because that file is at the size gate, and this is a
//! group with one subject — [`crate::song::Automation`]. The parent's match
//! hands every one of these variants here by name, as it does
//! [`super::length`]'s, and the catch-all below is there for the same reason
//! that one's is.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    /// The words for an automation refusal. Only ever called from
    /// [`SynthError::say`] with one of the seven variants it names.
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
            // `say` hands over only the seven above; see `length`'s catch-all.
            other => write!(f, "{other:?}"),
        }
    }
}
