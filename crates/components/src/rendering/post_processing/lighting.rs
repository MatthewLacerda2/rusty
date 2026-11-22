use serde::{Deserialize, Serialize};

use rusty_core::settings::quality::EffectQuality;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowSettings {
    /// `1..=4`; each cascade is one more depth pass.
    pub cascades: u32,
    /// World units; nothing casts or receives a sun shadow past it (fades over the last tenth).
    pub distance: f32,
}

impl ShadowSettings {
    pub const MAX_CASCADES: u32 = 4;
    pub const MIN_DISTANCE: f32 = 1.0;
}

impl Default for ShadowSettings {
    /// Unity's High tier is 4 cascades over 150 m; 100 m suits a shooter's closer fights.
    fn default() -> Self {
        Self {
            cascades: Self::MAX_CASCADES,
            distance: 100.0,
        }
    }
}

/// HDRP's Ambient Occlusion override: darkens only ambient/indirect light, never direct.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SsaoSettings {
    pub active: bool,
    /// Metres: how far from a surface another still shades it.
    pub radius: f32,
    /// `0` is none, `1` physically plausible, above `1` exaggerated.
    pub intensity: f32,
    pub quality: EffectQuality,
    /// `Custom`: samples per occlusion texel, `1..=64`.
    pub samples: u32,
    /// `Custom`: the view's size divided by this, `1..=4`.
    pub resolution_divisor: u32,
}

impl SsaoSettings {
    pub const MAX_SAMPLES: u32 = 64;
    pub const MIN_RADIUS: f32 = 0.01;
    pub const MAX_INTENSITY: f32 = 4.0;
}

impl Default for SsaoSettings {
    /// Half a metre at `Medium` (`Custom` starts at 8 samples, half resolution).
    fn default() -> Self {
        Self {
            active: true,
            radius: 0.5,
            intensity: 1.0,
            quality: EffectQuality::Medium,
            samples: 8,
            resolution_divisor: 2,
        }
    }
}
