//! What a square's pulse width is refused with: a width nothing can hear.
//!
//! Out of [`super`] because that file is at the size gate. The parent's match
//! hands both variants here by name, as it does [`super::timing`]'s, and the
//! catch-all below is there for the same reason that one's is.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    /// The words for a pulse-width refusal. Only ever called from
    /// [`SynthError::say`] with one of the two variants it names.
    pub(super) fn say_pulse(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PulseWidthOffSquare { wave } => write!(
                f,
                "patch: `pulse_width` is written on a `{}` oscillator, and only a `square` has one — set `wave` to `square` or leave the field out",
                format!("{wave:?}").to_lowercase()
            ),
            Self::NoPulseToModulate => write!(
                f,
                "patch: the lfo is aimed at `pulse_width`, and the source has no `square` oscillator for it to move"
            ),
            // `say` hands over only the two above; see `length`'s catch-all.
            other => write!(f, "{other:?}"),
        }
    }
}
