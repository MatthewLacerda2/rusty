//! src/components/rendering/post_processing/ — the post-processing volume, Unity's
//! Volume with its overrides: one settings struct per post effect (#1467), each in
//! its own file (grading effects under `grading/`, lens effects under `lens/`), plus
//! the volume's lighting settings (`lighting`), which are no post effect.
//!
//! A new effect adds its file, its `mod` line and one field below. The other layers
//! find it by that field's name (`render::postfx::effects`, whose completeness test
//! checks every layer has its part).

use serde::{Deserialize, Serialize};

// sorted: one module per effect (or family of effects); none reads another
mod bloom;
mod custom_effects;
mod depth_of_field;
#[cfg(test)]
pub mod dummy;
mod exposure;
mod grading;
mod lens;
mod lighting;
mod ssr;

// sorted: each module's settings, re-exported by name
pub use bloom::Bloom;
pub use custom_effects::CustomEffects;
pub use depth_of_field::{DepthOfField, DofMode};
pub use exposure::{AutoExposure, Exposure, ExposureMode};
pub use grading::{
    ColorAdjustments, ColorLookup, ColorWheel, LiftGammaGain, Tonemap, Tonemapping, Wheel,
    WhiteBalance,
};
pub use lens::{ChromaticAberration, FilmGrain, Vignette};
pub use lighting::{ShadowSettings, SsaoSettings};
pub use ssr::Ssr;

/// A post-processing volume. Every field defaults when a file leaves it out, so a
/// scene names only what it sets, each effect to its own default (the Add Component
/// menu's, `scene::authoring::default_post_processing`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PostProcessingComponent {
    /// Whether the volume applies at all: a camera whose volume is off runs neutral.
    #[serde(default = "on")]
    pub active: bool,
    // sorted: one field per post effect, named as every layer names the effect
    pub bloom: Bloom,
    pub chromatic_aberration: ChromaticAberration,
    pub color_adjustments: ColorAdjustments,
    pub color_lookup: ColorLookup,
    pub custom_effects: CustomEffects,
    pub depth_of_field: DepthOfField,
    #[cfg(test)]
    pub dummy: dummy::Dummy,
    pub exposure: Exposure,
    pub film_grain: FilmGrain,
    pub lift_gamma_gain: LiftGammaGain,
    pub ssr: Ssr,
    pub tonemapping: Tonemapping,
    pub vignette: Vignette,
    pub white_balance: WhiteBalance,

    /// Directional-shadow cascades and reach (#435).
    pub shadows: ShadowSettings,
    /// Screen-space ambient occlusion (#436), on by default.
    pub ssao: SsaoSettings,
}

impl PostProcessingComponent {
    /// The fields that are no post effect: the volume's own switch and its lighting
    /// settings. Every other field is one effect's settings.
    pub const NOT_EFFECTS: [&'static str; 3] = ["active", "shadows", "ssao"];
}

/// A loaded volume that leaves `active` out applies.
fn on() -> bool {
    true
}

#[cfg(test)]
mod tests;
