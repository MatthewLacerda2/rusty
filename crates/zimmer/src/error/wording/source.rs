//! What a patch's sources are refused with, and its layers: an empty, oversized
//! or silent oscillator stack or additive series, a ratio that names no
//! frequency, an `fm4` heard through silent carriers, too many layers, a
//! silent one, and a layer whose own stage was refused.
//!
//! Out of [`super`] for the reason [`length`](super::length) is: that file is
//! at the size gate, and a source is the seam the instrument group already
//! had. The parent's match hands every one of these variants here by name;
//! one that reaches the catch-all arm below prints its `Debug` form rather
//! than panicking inside a `Display`.

use std::fmt;

use crate::error::SynthError;

impl SynthError {
    /// The words for a source or layer refusal. Only ever called from
    /// [`SynthError::say`] with one of the variants it names.
    pub(super) fn say_source(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOscStack => write!(f, "patch: `osc_stack` needs at least one oscillator"),
            Self::TooManyOscs { found, limit, .. } => write!(
                f,
                "patch: `osc_stack` takes at most {limit} oscillators, got {found}"
            ),
            Self::SilentOscStack => write!(
                f,
                "patch: every oscillator gain is zero — the stack is silent"
            ),
            Self::BadVoiceCount { found, limit, .. } => write!(
                f,
                "patch: an oscillator sounds 1 to {limit} voices, got {found}"
            ),
            Self::BadFmRatio { ratio, .. } => {
                write!(f, "patch: `fm2` needs a positive `ratio`, got {ratio}")
            }
            Self::EmptyPartials => write!(f, "patch: `additive` needs at least one partial"),
            Self::TooManyPartials { found, limit, .. } => write!(
                f,
                "patch: `additive` takes at most {limit} partials, got {found}"
            ),
            Self::SilentPartials => write!(
                f,
                "patch: every partial gain is zero — the series is silent"
            ),
            Self::BadPartialRatio { index, ratio, .. } => write!(
                f,
                "patch: `additive` partial {index} needs a positive `ratio`, got {ratio}"
            ),
            Self::BadOperatorRatio {
                operator, ratio, ..
            } => write!(
                f,
                "patch: `fm4` operator {operator} needs a positive `ratio`, got {ratio}"
            ),
            Self::SilentCarriers { algorithm, .. } => write!(
                f,
                "patch: `fm4` algorithm `{algorithm}` is heard through its carriers, and every one of them is at level zero"
            ),
            Self::TooManyLayers { found, limit, .. } => write!(
                f,
                "patch: `layers` takes at most {limit} layers, got {found}"
            ),
            Self::SilentLayer => write!(
                f,
                "patch: a layer needs a positive `gain` — this one is silent"
            ),
            Self::InLayer { layer, error, .. } => {
                let said = error.to_string();
                let said = said.strip_prefix("patch: ").unwrap_or(&said);
                write!(f, "patch: `layers[{layer}]`: {said}")
            }
            other => write!(f, "{other:?}"),
        }
    }
}
