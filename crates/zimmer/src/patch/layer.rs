//! Layers: more than one sound in one note.

use serde::{Deserialize, Serialize};

use super::{Adsr, Filter, PitchEnv, Source};
use crate::error::SynthError;

pub const MAX_LAYERS: usize = 3;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub source: Source,
    pub amp: Adsr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Filter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_env: Option<PitchEnv>,
    #[serde(default = "unity", skip_serializing_if = "is_unity")]
    pub gain: f32,
}

fn unity() -> f32 {
    1.0
}

fn is_unity(gain: &f32) -> bool {
    *gain == 1.0
}

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
