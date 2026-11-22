use serde::{Deserialize, Serialize};

/// Animated by the game clock (`Scene::shader_time`), so a capture at a given tick is reproducible.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilmGrain {
    pub active: bool,
    /// `0..=1`; `0` draws nothing.
    pub intensity: f32,
    /// Fade in bright areas, `0..=1`: `0` grains every pixel alike, `1` leaves highlights clean.
    pub response: f32,
}

impl FilmGrain {
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
