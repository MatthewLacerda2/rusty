use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomEffects {
    pub names: Vec<String>,
    /// Unity's volume-profile overrides without the blending, by the name a script set them under
    /// (`"damage_vignette.intensity"` → `[0.7]`); an effect whose layout lacks a name keeps its
    /// baked default.
    pub params: BTreeMap<String, Vec<f32>>,
}
