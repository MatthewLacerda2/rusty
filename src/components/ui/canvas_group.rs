//! src/components/ui/canvas_group.rs — CanvasGroup: a UI subtree's alpha and interaction (#418).
//!
//! Unity's `CanvasGroup`. Its `alpha` multiplies every graphic on this entity and
//! its descendants (nested groups multiply) — screen fades, disabled panels. The
//! two flags are read by pointer dispatch (#420): `interactable = false` disables
//! the subtree's selectables, `blocks_raycasts = false` lets the pointer pass
//! through it. Pure authoring data.

use serde::{Deserialize, Serialize};

/// A UI subtree's opacity and interaction switches. See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasGroupComponent {
    /// Opacity multiplied into every graphic below (this entity included). Kept in
    /// `[0, 1]`.
    pub alpha: f32,
    /// Whether the subtree's selectables accept input (read by #420).
    pub interactable: bool,
    /// Whether the subtree's graphics can be hit by the pointer (read by #420).
    pub blocks_raycasts: bool,
}

impl Default for CanvasGroupComponent {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            interactable: true,
            blocks_raycasts: true,
        }
    }
}
