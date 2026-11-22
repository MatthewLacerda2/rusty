use serde::{Deserialize, Serialize};

// sorted: one module per effect (or family of effects); none reads another
mod bloom;
mod custom_effects;
mod depth_of_field;

impl crate::FirstClass for PostProcessingComponent {
    const NAME: &'static str = "PostProcessing";
}

mod exposure;
mod grading;
mod lens;
mod lens_flare;
mod lighting;
mod ssil;
mod ssr;
mod volumetric_fog;

// sorted: each module's settings, re-exported by name
pub use bloom::{Bloom, DEFAULT_DIRT_TEXTURE};
pub use custom_effects::CustomEffects;
pub use depth_of_field::{DepthOfField, DofMode};
pub use exposure::{AutoExposure, Exposure, ExposureMode};
pub use grading::{
    ColorAdjustments, ColorLookup, ColorWheel, LiftGammaGain, Tonemap, Tonemapping, Wheel,
    WhiteBalance,
};
pub use lens::{ChromaticAberration, FilmGrain, Vignette};
pub use lens_flare::LensFlare;
pub use lighting::{ShadowSettings, SsaoSettings};
pub use ssil::Ssil;
pub use ssr::Ssr;
pub use volumetric_fog::VolumetricFog;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PostProcessingComponent {
    #[serde(default = "on")]
    pub active: bool,
    // sorted: one field per post effect, named as every layer names the effect
    pub bloom: Bloom,
    pub chromatic_aberration: ChromaticAberration,
    pub color_adjustments: ColorAdjustments,
    pub color_lookup: ColorLookup,
    pub custom_effects: CustomEffects,
    pub depth_of_field: DepthOfField,
    pub exposure: Exposure,
    pub film_grain: FilmGrain,
    pub lens_flare: LensFlare,
    pub lift_gamma_gain: LiftGammaGain,
    pub ssil: Ssil,
    pub ssr: Ssr,
    pub tonemapping: Tonemapping,
    pub vignette: Vignette,
    pub volumetric_fog: VolumetricFog,
    pub white_balance: WhiteBalance,

    pub shadows: ShadowSettings,
    pub ssao: SsaoSettings,
}

impl Default for PostProcessingComponent {
    fn default() -> Self {
        Self {
            active: on(),
            shadows: Default::default(),
            ssao: Default::default(),
            // sorted: one line per post effect, as the struct lists its fields
            bloom: Default::default(),
            chromatic_aberration: Default::default(),
            color_adjustments: Default::default(),
            color_lookup: Default::default(),
            custom_effects: Default::default(),
            depth_of_field: Default::default(),
            exposure: Default::default(),
            film_grain: Default::default(),
            lens_flare: Default::default(),
            lift_gamma_gain: Default::default(),
            ssil: Default::default(),
            ssr: Default::default(),
            tonemapping: Default::default(),
            vignette: Default::default(),
            volumetric_fog: Default::default(),
            white_balance: Default::default(),
        }
    }
}

impl PostProcessingComponent {
    pub const NOT_EFFECTS: [&'static str; 3] = ["active", "shadows", "ssao"];
}

fn on() -> bool {
    true
}

#[cfg(test)]
mod tests;
