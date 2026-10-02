//! src/asset/animation_graph/layer.rs — animation layers (#457).
//!
//! A layer is a further state machine blended over everything below it: Unity's
//! `AnimatorControllerLayer`. Its **weight** says how much it shows, its
//! **blending** whether it replaces the pose below (`Override`) or adds its
//! motion's change on top (`Additive`), and its **mask** which bones it touches —
//! so an upper-body layer fires and reloads while the base layer's legs keep
//! running, and an additive flinch stacks on any pose.

use serde::{Deserialize, Serialize};

use super::StateMachine;

/// How a layer combines with the pose below it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerBlending {
    /// Blend toward this layer's pose by the layer weight (1 replaces it).
    #[default]
    Override,
    /// Add this layer's motion, measured against the motion's own first frame
    /// (Unity's default additive reference pose), scaled by the layer weight.
    Additive,
}

/// One extra layer over the base layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GraphLayer {
    /// The layer's name — how scripts address it besides its index.
    pub name: String,
    /// The layer's starting weight in `[0, 1]`; scripts change it at runtime
    /// (`Animator.SetLayerWeight`). Unity's default for a new layer is 0, but an
    /// authored layer usually means "on", so the asset default is 1.
    #[serde(default = "full_weight")]
    pub weight: f32,
    #[serde(default)]
    pub blending: LayerBlending,
    /// The avatar mask: bone names, each standing for that bone and its whole
    /// subtree (`["spine_01"]` is "spine and up"). Empty means the whole body.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mask: Vec<String>,
    /// The layer's own states, transitions and entry.
    #[serde(flatten)]
    pub machine: StateMachine,
}

fn full_weight() -> f32 {
    1.0
}
