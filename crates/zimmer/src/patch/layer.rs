//! Layers: more than one sound in one note.
//!
//! Most struck instruments are two things at once with **different** shapes —
//! a snare's noise crack over a pitched body, a kick's click over a falling
//! sine, a hammer over a string. A [`Layer`] is one more of those: its own
//! source, filter, amp envelope and pitch envelope, run exactly as the patch's
//! own are, and **added** to them before the patch's one fx chain.
//!
//! Still not a free graph: the sum is fixed and parallel. No layer feeds
//! another, a layer has no chain of its own, and a layer holds no layers.
//! What the layers share is the note — its pitch, velocity, timbre, glide,
//! gate and the patch's LFOs — and each draws its own randomness, so two noise
//! layers are two noises rather than one played twice.

use serde::{Deserialize, Serialize};

use super::{Adsr, Filter, PitchEnv, Source};
use crate::error::SynthError;

/// The most layers a patch may add to its own sound: four sounds in all.
/// Past that a note costs a source and a filter per layer for a texture no
/// struck instrument needs, so the cap is stated rather than discovered.
pub const MAX_LAYERS: usize = 3;

/// One more sound in the note, summed with the patch's own before its fx.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    /// What makes this layer's tone.
    pub source: Source,
    /// This layer's amplitude envelope.
    pub amp: Adsr,
    /// This layer's own filter. A song's `cutoff` curve never moves it: that
    /// moves the patch's top-level filter only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    /// This layer's own sweep of the note's pitch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_env: Option<PitchEnv>,
    /// How loud the layer sits against the patch's own sound, as a linear
    /// factor. Unity when absent.
    #[serde(default = "unity", skip_serializing_if = "is_unity")]
    pub gain: f32,
}

fn unity() -> f32 {
    1.0
}

/// Whether a layer's gain is the default, so a document that never wrote one
/// is not given one on the way back out.
fn is_unity(gain: &f32) -> bool {
    *gain == 1.0
}

/// Rejects a set of layers the renderer cannot honour: too many, one that is
/// silent, or one whose own source or filter is refused — each named by its
/// place in `layers`, counted from zero.
pub(super) fn check_layers(layers: &[Layer]) -> Result<(), SynthError> {
    if layers.len() > MAX_LAYERS {
        return Err(SynthError::TooManyLayers {
            found: layers.len(),
            limit: MAX_LAYERS,
        });
    }
    for (layer, written) in layers.iter().enumerate() {
        let checked = if written.gain.is_finite() && written.gain > 0.0 {
            super::check_stages(&written.source, written.filter.as_ref())
        } else {
            Err(SynthError::SilentLayer)
        };
        checked.map_err(|error| SynthError::InLayer {
            layer,
            error: Box::new(error),
        })?;
    }
    Ok(())
}
