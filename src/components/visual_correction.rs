//! src/components/visual_correction.rs — Post-process component
//!
//! bloom/exposure/SSR settings. Unity: post-process volume. Moved verbatim from
//! the legacy `core/scene.rs`, then extended (phase-3) so the renderer's post-FX
//! chain (color correction, bloom, motion blur, real SSR) actually reads them.

use serde::{Deserialize, Serialize};

/// Tonemapping operator applied as the final color-correction step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tonemap {
    /// No tonemap — clamp only. Mostly useful for debugging.
    None,
    /// Reinhard `c / (1 + c)` — cheap, soft rolloff.
    Reinhard,
    /// Filmic ACES fit — the default, richer contrast in highlights.
    #[default]
    Aces,
}

impl Tonemap {
    /// Stable index handed to the shader as a `u32`.
    pub fn to_index(self) -> u32 {
        match self {
            Tonemap::None => 0,
            Tonemap::Reinhard => 1,
            Tonemap::Aces => 2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualCorrectionComponent {
    pub active: bool,
    pub bloom_active: bool,
    pub bloom_intensity: f32,
    pub bloom_threshold: f32,
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub ssr_active: bool,
    pub ssr_quality: String, // "Low", "Medium", "High", "Ultra"
    pub ssr_temporal_upsampling: bool,
    /// Tonemap operator. `#[serde(default)]` keeps older scene files loading.
    #[serde(default)]
    pub tonemap: Tonemap,
    /// Output gamma for the final encode (2.2 is the usual sRGB-ish value).
    #[serde(default = "default_gamma")]
    pub gamma: f32,
}

fn default_gamma() -> f32 {
    2.2
}
