//! What a square's pulse width is refused with: a width nothing can hear.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
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
            _ => unreachable!("`say_pulse` is handed only the refusals it words"),
        }
    }
}
