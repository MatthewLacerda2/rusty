//! Screen-space reflections, Unity HDRP's *Screen Space Reflection* override: a scene
//! pass right after the base camera's opaque pass, ahead of the translucents and every
//! post effect. The quality sets the ray march: its step count and step length
//! (`render::postfx::effects::ssr`).

use serde::{Deserialize, Serialize};

use crate::core::settings::quality::EffectQuality;

/// Whether SSR runs, and its march.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ssr {
    pub active: bool,
    /// Which march runs (#1522): a level of the active quality preset, or `steps`
    /// and `step_length` here when `Custom`.
    pub quality: EffectQuality,
    /// `Custom`: how many steps the ray takes, `1..=256`.
    pub steps: u32,
    /// `Custom`: each step's length in world units, `0.05..=2`. The reach is
    /// `steps × step_length`.
    pub step_length: f32,
}

impl Ssr {
    /// The most steps a write accepts.
    pub const MAX_STEPS: u32 = 256;
    /// The shortest and longest step a write accepts.
    pub const MIN_STEP_LENGTH: f32 = 0.05;
    pub const MAX_STEP_LENGTH: f32 = 2.0;
}

impl Default for Ssr {
    /// On, at the preset's `Medium` level; `Custom` starts at HDRP's Medium march,
    /// 32 steps of 0.375 m.
    fn default() -> Self {
        Self {
            active: true,
            quality: EffectQuality::Medium,
            steps: 32,
            step_length: 0.375,
        }
    }
}
