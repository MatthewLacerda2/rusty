//! Lens flare (#1443), Unity URP's *Screen Space Lens Flare* override: built from
//! bloom's blurred bright pass, so every bright pixel flares with no per-light
//! authoring (`render::postfx::effects::lens_flare`). It runs only when bloom runs.

use serde::{Deserialize, Serialize};

/// Ghosts mirrored through the screen centre, a halo ring and anamorphic streaks.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LensFlare {
    /// Whether the flare draws at all.
    pub active: bool,
    /// The whole flare's strength, `≥ 0`; `0` draws nothing.
    pub intensity: f32,
    /// How many ghosts mirror each bright pixel, `0..=MAX_GHOSTS`.
    pub ghost_count: u32,
    /// The ghosts' strength, `≥ 0`.
    pub ghost_intensity: f32,
    /// The halo ring's strength, `≥ 0`.
    pub halo_intensity: f32,
    /// The horizontal (anamorphic) streaks' strength, `≥ 0`; off at `0`.
    pub streak_intensity: f32,
    /// The colour the flare is multiplied by, linear RGB `0..=1`.
    pub tint: [f32; 3],
    /// How far the flare's red and blue split from its green, `0..=1`.
    pub chromatic_aberration: f32,
}

impl LensFlare {
    /// The most ghosts a flare draws: each is one tap per channel.
    pub const MAX_GHOSTS: u32 = 8;
    /// Off; once on, four ghosts, a soft halo and no streaks, untinted.
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
