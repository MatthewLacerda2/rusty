use serde::{Deserialize, Serialize};

mod partial;
mod presets;
mod seeds;

pub use presets::{QualityPreset, QualityPresets};

crate::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum EffectQuality {
        Low = "Low",
        #[default]
        Medium = "Medium",
        High = "High",
        Custom = "Custom",
    }
}

impl EffectQuality {
    pub fn pick<T: Copy>(self, levels: &Levels<T>, custom: T) -> T {
        match self {
            EffectQuality::Low => levels.low,
            EffectQuality::Medium => levels.medium,
            EffectQuality::High => levels.high,
            EffectQuality::Custom => custom,
        }
    }
}

/// The coarsest working resolution a divisor may ask for: a quarter of the view's size.
pub const MAX_RESOLUTION_DIVISOR: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Levels<T> {
    pub low: T,
    pub medium: T,
    pub high: T,
}

pub(super) const fn levels<T>(low: T, medium: T, high: T) -> Levels<T> {
    Levels { low, medium, high }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaoQuality {
    pub samples: u32,
    pub resolution_divisor: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsilQuality {
    pub samples: u32,
    pub resolution_divisor: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SsrQuality {
    pub steps: u32,
    /// The longest step in world units; `ssr_march.wgsl`'s `step_cap` also holds each
    /// to `max(1/64, 1/steps)` of the screen's shorter side.
    pub step_length: f32,
    /// *Mixed*: the traced rays run at this divisor of the march's resolution (1 full, 2 half;
    /// at most `MAX_RESOLUTION_DIVISOR`).
    pub trace_resolution_divisor: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumetricFogQuality {
    pub resolution: u32,
    pub slices: u32,
}

/// Unity's `QualitySettings.streamingMipmapsActive` and `streamingMipmapsMemoryBudget`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextureStreaming {
    pub active: bool,
    /// Megabytes of textures resident at once, the streamed and the rest together.
    pub budget_mb: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityValues {
    #[serde(deserialize_with = "partial::ssao")]
    pub ssao: Levels<SsaoQuality>,
    #[serde(deserialize_with = "partial::ssil")]
    pub ssil: Levels<SsilQuality>,
    #[serde(deserialize_with = "partial::ssr")]
    pub ssr: Levels<SsrQuality>,
    #[serde(deserialize_with = "partial::bloom")]
    pub bloom_resolution_divisor: Levels<u32>,
    #[serde(deserialize_with = "partial::depth_of_field")]
    pub depth_of_field_resolution_divisor: Levels<u32>,
    #[serde(deserialize_with = "partial::motion_blur")]
    pub motion_blur_samples: Levels<u32>,
    #[serde(deserialize_with = "partial::volumetric_fog")]
    pub volumetric_fog: Levels<VolumetricFogQuality>,
    pub ui_blur_divisor: u32,
    pub shadowmask_mode: ShadowmaskMode,
    pub gpu_occlusion_culling: bool,
    pub gpu_cluster_culling: bool,
    #[serde(deserialize_with = "partial::texture_streaming")]
    pub texture_streaming: TextureStreaming,
}

crate::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ShadowmaskMode {
        #[default]
        Shadowmask = "Shadowmask",
        DistanceShadowmask = "DistanceShadowmask",
    }
}

impl QualityValues {
    pub fn backdrop_divisor(&self) -> u32 {
        self.ui_blur_divisor.clamp(1, 8).next_power_of_two()
    }
}

impl Default for QualityValues {
    fn default() -> Self {
        seeds::medium()
    }
}

#[cfg(test)]
mod tests;
