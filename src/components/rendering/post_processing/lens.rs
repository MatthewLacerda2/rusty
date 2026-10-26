//! The lens and film effects of a post-processing volume (#1439): Vignette,
//! Chromatic Aberration and Film Grain, Unity URP/HDRP's volume overrides of the same
//! names with their parameters. Each is off until its `active` flag is set; its
//! strengths then start at a visible amount (Unity's start at `0`, which reads as
//! "broken" when an agent ticks the box and sees nothing).
//!
//! Where they run in the composite (`engine/shaders/postfx/lens.wgsl`): chromatic
//! aberration samples the HDR image before the grade, the vignette darkens the
//! graded image, and film grain is the last thing before the authored effects and
//! FXAA.

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

/// Splits the colour channels towards the screen edges, as a cheap lens does,
/// Unity's *Chromatic Aberration*: radial, nothing at the centre.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromaticAberration {
    /// Whether the fringes draw at all.
    pub active: bool,
    /// How far the channels split at the screen's corners, `0..=1`.
    pub intensity: f32,
}

impl ChromaticAberration {
    /// Off; once on, a third-strength split.
    pub const DEFAULT: Self = Self {
        active: false,
        intensity: 0.3,
    };
}

impl Default for ChromaticAberration {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Per-pixel film noise, Unity's *Film Grain*. Animated by the game clock
/// (`Scene::shader_time`), so a capture at a given tick is reproducible and an
/// edit-mode capture (time `0`) is stable.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilmGrain {
    /// Whether the grain draws at all.
    pub active: bool,
    /// How strong the grain is, `0..=1`; `0` draws nothing.
    pub intensity: f32,
    /// How much the grain fades out in bright areas, `0..=1`: `0` grains every
    /// pixel alike, `1` leaves the highlights clean.
    pub response: f32,
}

impl FilmGrain {
    /// Off; once on, a third-strength grain with Unity's default response.
    pub const DEFAULT: Self = Self {
        active: false,
        intensity: 0.3,
        response: 0.8,
    };
}

impl Default for FilmGrain {
    fn default() -> Self {
        Self::DEFAULT
    }
}
