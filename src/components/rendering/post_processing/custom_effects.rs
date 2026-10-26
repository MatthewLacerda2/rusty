//! The authored post-FX chain (#397): `Shader.Bake` modules with `pass = "postfx"`,
//! run after the tonemap and before FXAA, and their runtime params (#671).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The authored effects a volume runs, in order, and their runtime params.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomEffects {
    /// Each a module *name*, the file the renderer loads from the authored-shader
    /// workspace, run in this order.
    pub names: Vec<String>,
    /// Runtime params of the effects, by the name a script set them under
    /// (`"damage_vignette.intensity"` → `[0.7]`): Unity's volume-profile overrides,
    /// without the blending. Every effect in `names` whose layout resolves a name
    /// draws with its value; the rest keep their baked defaults.
    pub params: BTreeMap<String, Vec<f32>>,
}
