//! src/core/settings/quality/ — quality presets: the game's named bundles of
//! per-effect quality values (#1522), HDRP's model.
//!
//! Every post effect with a cost knob carries a [`EffectQuality`] selector on the
//! camera or volume. `Low` / `Medium` / `High` take that effect's values from the
//! **active preset** (its row for that level); `Custom` uses the values authored on
//! the volume beside the selector. So a preset writes the defaults a volume opts
//! into, and a volume can always say exactly what it wants.
//!
//! The presets are the game's, as Unity's Project Settings → Quality levels are:
//! they live in `project.rusty` (`"quality"`, [`QualityPresets`]), a new project is
//! seeded with `Low`, `Medium` and `High` (`seeds`), and the game may edit, rename,
//! add to or delete them. The engine gates no effect on a preset: an effect that is
//! on runs, at the quality its settings say. Plain data, no GPU types — the render
//! layer reads the values (#494).

use serde::{Deserialize, Serialize};

mod presets;
mod seeds;

pub use presets::{QualityPreset, QualityPresets};

crate::core::named_enum! {
    /// Which values an effect's cost knobs take: one of the active preset's three
    /// levels, or the ones authored beside the selector.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum EffectQuality {
        Low = "Low",
        /// HDRP's default level.
        #[default]
        Medium = "Medium",
        High = "High",
        /// The values on the camera or volume itself.
        Custom = "Custom",
    }
}

impl EffectQuality {
    /// The effect's value at this level: `levels`' row, or `custom` for `Custom`.
    pub fn pick<T: Copy>(self, levels: &Levels<T>, custom: T) -> T {
        match self {
            EffectQuality::Low => levels.low,
            EffectQuality::Medium => levels.medium,
            EffectQuality::High => levels.high,
            EffectQuality::Custom => custom,
        }
    }
}

/// The coarsest working resolution a quality value may ask for: a quarter.
pub const MAX_RESOLUTION_DIVISOR: u32 = 4;

/// One effect's values at a preset's three levels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Levels<T> {
    pub low: T,
    pub medium: T,
    pub high: T,
}

pub(super) const fn levels<T>(low: T, medium: T, high: T) -> Levels<T> {
    Levels { low, medium, high }
}

/// The SSAO pass's cost (#436): its sample count and working resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsaoQuality {
    /// Hemisphere samples traced per occlusion texel.
    pub samples: u32,
    /// The occlusion target is the view's size divided by this (1 = full).
    pub resolution_divisor: u32,
}

/// The SSR ray march (#1521): how many steps, how long each in world units. Its
/// reach is their product.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SsrQuality {
    pub steps: u32,
    pub step_length: f32,
}

/// Every per-effect quality value one preset holds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityValues {
    pub ssao: Levels<SsaoQuality>,
    pub ssr: Levels<SsrQuality>,
    /// Bloom's buffer is the view's size divided by this.
    pub bloom_resolution_divisor: Levels<u32>,
    /// Depth of field blurs at the view's size divided by this (#1445).
    pub depth_of_field_resolution_divisor: Levels<u32>,
    /// Motion blur's taps per pixel (#1436).
    pub motion_blur_samples: Levels<u32>,
    /// The UI backdrop blur's working-resolution divisor (#426), a power of two. The
    /// UI has no volume, so it takes one value, not three levels.
    pub ui_blur_divisor: u32,
}

impl QualityValues {
    /// The UI backdrop blur's divisor, as the blur takes it: a power of two, `1..=8`.
    pub fn backdrop_divisor(&self) -> u32 {
        self.ui_blur_divisor.clamp(1, 8).next_power_of_two()
    }
}

impl Default for QualityValues {
    /// The seeded `Medium` preset's values: what runs when a game defines none.
    fn default() -> Self {
        seeds::medium()
    }
}

#[cfg(test)]
mod tests;
