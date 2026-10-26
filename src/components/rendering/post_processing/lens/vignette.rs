//! Vignette (#1439), Unity's override of that name: run on the graded image
//! (`engine/shaders/postfx/effects/vignette.wgsl`).

use serde::{Deserialize, Serialize};

/// Darkens (or tints) the image towards its edges, Unity's *Vignette*.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Vignette {
    /// Whether the vignette draws at all.
    pub active: bool,
    /// How strongly the edges darken, `0..=1`; `0` draws nothing.
    pub intensity: f32,
    /// How soft the falloff from the clear centre to the edge is,
    /// `MIN_SMOOTHNESS..=1`.
    pub smoothness: f32,
    /// The colour the edges fade to, linear RGB `0..=1`; black by default.
    pub color: [f32; 3],
    /// The clear centre, in screen uv (`0, 0` top-left, `1, 1` bottom-right).
    pub center: [f32; 2],
}

impl Vignette {
    /// The hardest falloff a write accepts: Unity's floor, short of a hard edge.
    pub const MIN_SMOOTHNESS: f32 = 0.01;
    /// Off; once on, a third-strength black vignette with Unity's default falloff,
    /// centred on the screen.
    pub const DEFAULT: Self = Self {
        active: false,
        intensity: 0.3,
        smoothness: 0.2,
        color: [0.0; 3],
        center: [0.5, 0.5],
    };
}

impl Default for Vignette {
    fn default() -> Self {
        Self::DEFAULT
    }
}
