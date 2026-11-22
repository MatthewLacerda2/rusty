use serde::{Deserialize, Serialize};

use super::StateMachine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerBlending {
    #[default]
    Override,
    /// Add this layer's motion, measured against the motion's own first frame
    /// (Unity's default additive reference pose), scaled by the layer weight.
    Additive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphLayer {
    pub name: String,
    /// Starting weight in `[0, 1]`. Unity's default for a new layer is 0, but an authored layer
    /// usually means "on", so 1.
    #[serde(default = "full_weight")]
    pub weight: f32,
    #[serde(default)]
    pub blending: LayerBlending,
    /// Bone names, each covering its whole subtree (`["spine_01"]` is "spine and up"); empty is
    /// the whole body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask: Vec<String>,
    #[serde(flatten)]
    pub machine: StateMachine,
}

fn full_weight() -> f32 {
    1.0
}
