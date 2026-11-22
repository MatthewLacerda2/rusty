//! Unity URP's Bloom, with an optional lens dirt texture.

use serde::{Deserialize, Serialize};

use rusty_core::settings::quality::EffectQuality;

/// A neutral procedural smudge texture for games without dirt art.
pub const DEFAULT_DIRT_TEXTURE: &str = "engine/textures/lens_dirt.png";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bloom {
    pub active: bool,
    /// `≥ 0`.
    pub intensity: f32,
    /// The brightest channel above which a pixel blooms, `≥ 0`; soft knee half its width either
    /// side, as URP.
    pub threshold: f32,
    /// `0..=1`, URP's *Scatter*.
    pub scatter: f32,
    /// A project-relative path (or [`DEFAULT_DIRT_TEXTURE`], or `rt:`), cropped to cover the
    /// screen.
    pub dirt_texture: Option<String>,
    /// `≥ 0`.
    pub dirt_intensity: f32,
    pub quality: EffectQuality,
    /// `Custom`: the view's size divided by this, `1..=4`.
    pub resolution_divisor: u32,
}

impl Default for Bloom {
    /// Unity's defaults, at the preset's `Medium` level (`Custom` starts at half resolution).
    fn default() -> Self {
        Self {
            active: true,
            intensity: 1.0,
            threshold: 0.8,
            scatter: 0.7,
            dirt_texture: None,
            dirt_intensity: 0.0,
            quality: EffectQuality::Medium,
            resolution_divisor: 2,
        }
    }
}
