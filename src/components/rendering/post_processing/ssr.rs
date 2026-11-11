//! Screen-space reflections, Unity HDRP's *Screen Space Reflection* override: a scene
//! pass right after the base camera's opaque pass, ahead of the translucents and every
//! post effect. The quality sets the ray march: its step count and step length
//! (`render::postfx::effects::ssr`). A rough surface reflects a blur of its hits,
//! averaged over the width of its GGX lobe (#1736, #2028), down to the *Minimum
//! Smoothness*.

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
    /// `Custom`: each step's longest length in world units, `0.05..=2`. A step
    /// is also held to `max(1/64, 1/steps)` of the screen's shorter side (#2004),
    /// so the reach is at most `steps × step_length`, less where the ray crosses
    /// the screen fast, and always enough to cross the screen's height.
    pub step_length: f32,
    /// HDRP's *Minimum Smoothness*, `0..=1`: a surface less smooth than this
    /// (smoothness is `1 − roughness`) gets no SSR and keeps its probe or sky
    /// reflection; SSR fades in over the next [`Ssr::SMOOTHNESS_FADE`] of
    /// smoothness, narrowed so it is whole by smoothness 0.9 (HDRP's default
    /// *Smoothness Fade Start*: 0.8 fades in over 0.1, SSR's look before this knob).
    /// From 0.96, the smoothest any surface gets, the view runs no SSR. The default
    /// 0.3 reflects up to roughness 0.7, fully to 0.5.
    pub min_smoothness: f32,
}

impl Ssr {
    /// The most steps a write accepts.
    pub const MAX_STEPS: u32 = 256;
    /// The shortest and longest step a write accepts.
    pub const MIN_STEP_LENGTH: f32 = 0.05;
    pub const MAX_STEP_LENGTH: f32 = 2.0;
    /// How much smoothness above `min_smoothness` SSR takes to fade fully in, at
    /// most (HDRP's *Smoothness Fade Start* is `min_smoothness` plus this, up to 0.9:
    /// `render::postfx::effects::ssr::fade`).
    pub const SMOOTHNESS_FADE: f32 = 0.2;
}

impl Default for Ssr {
    /// On, at the preset's `Medium` level; `Custom` starts at HDRP's Medium march,
    /// 32 steps of 0.375 m; reflecting up to roughness 0.7.
    fn default() -> Self {
        Self {
            active: true,
            quality: EffectQuality::Medium,
            steps: 32,
            step_length: 0.375,
            min_smoothness: 0.3,
        }
    }
}
