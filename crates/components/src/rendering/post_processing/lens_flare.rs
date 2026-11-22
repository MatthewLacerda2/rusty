//! Unity URP's *Screen Space Lens Flare*: built from bloom's prefiltered first mip, so it runs only
//! when bloom runs (`render::postfx::effects::lens_flare`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LensFlare {
    pub active: bool,
    /// `≥ 0`; `0` draws nothing.
    pub intensity: f32,
    pub ghost_count: u32,
    pub ghost_intensity: f32,
    pub halo_intensity: f32,
    /// `≥ 0`; off at `0`.
    pub streak_intensity: f32,
    /// Linear RGB, `0..=1`.
    pub tint: [f32; 3],
    /// How far red and blue split from green, `0..=1`.
    pub chromatic_aberration: f32,
}

impl LensFlare {
    /// Each ghost is one tap per channel.
    pub const MAX_GHOSTS: u32 = 8;
    pub const DEFAULT: Self = Self {
        active: false,
        intensity: 1.0,
        ghost_count: 4,
        ghost_intensity: 1.0,
        halo_intensity: 0.5,
        streak_intensity: 0.0,
        tint: [1.0; 3],
        chromatic_aberration: 0.5,
    };
}

impl Default for LensFlare {
    fn default() -> Self {
        Self::DEFAULT
    }
}
