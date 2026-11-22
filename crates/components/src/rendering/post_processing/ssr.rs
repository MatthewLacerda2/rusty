//! Unity HDRP's *Screen Space Reflection*, a pass right after the opaque pass. A rough surface
//! reflects a blur of its hits across its GGX lobe.

use serde::{Deserialize, Serialize};

use rusty_core::settings::quality::EffectQuality;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ssr {
    pub active: bool,
    pub quality: EffectQuality,
    pub steps: u32,
    /// `Custom`: longest step, `0.05..=2` world units; also held to `max(1/64, 1/steps)` of the
    /// screen's shorter side.
    pub step_length: f32,
    /// HDRP's *Minimum Smoothness*, `0..=1` (smoothness is `1 − roughness`): a rougher surface
    /// keeps its probe or sky reflection.
    pub min_smoothness: f32,
}

impl Ssr {
    pub const MAX_STEPS: u32 = 256;
    pub const MIN_STEP_LENGTH: f32 = 0.05;
    pub const MAX_STEP_LENGTH: f32 = 2.0;
    /// The most smoothness above `min_smoothness` SSR takes to fade in; `ssr::fade` narrows it
    /// to end by its `FADE_START`.
    pub const SMOOTHNESS_FADE: f32 = 0.2;
}

impl Default for Ssr {
    /// `Custom` starts at HDRP's Medium march, 32 steps of 0.375 m.
    fn default() -> Self {
        Self {
            active: true,
            quality: EffectQuality::Medium,
            steps: 32,
            step_length: 0.375,
            min_smoothness: 0.7,
        }
    }
}
