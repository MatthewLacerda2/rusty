//! Film Grain (#1439), Unity's override of that name: the last thing the composite
//! draws (`engine/shaders/postfx/effects/film_grain.wgsl`).

use serde::{Deserialize, Serialize};

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
