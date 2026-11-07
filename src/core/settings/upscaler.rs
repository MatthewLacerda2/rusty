//! src/core/settings/upscaler.rs — `Upscaler`: which temporal upscaler a screen
//! camera with TAA on runs (#1848), and where the choice comes from.
//!
//! URP's *Upscaling Filter*, on the pipeline asset beside *Render Scale*: `taa`
//! is TAA's own resolve (and TAAU's upscale below a render scale of 1), `fsr2`
//! AMD's FidelityFX Super Resolution 2 in its place, at any scale (1 included,
//! FSR's native anti-aliasing). A camera with TAA off upscales bilinearly under
//! either: that is the bilinear fallback, no third value. Render-only.
//!
//! The upscaler that runs is an [`UpscalerSetting`]: the player's choice,
//! persisted beside the render scale (`Storage` `graphics.upscaler`), over the
//! project's default (`project.rusty`'s `"graphics"` block,
//! [`ProjectGraphics`](super::project_graphics::ProjectGraphics)). With no choice
//! stored the default runs, so `Graphics.ResetUpscaler` brings the default back.

use serde::{Deserialize, Serialize};

use super::GRAPHICS_NAMESPACE;
use crate::core::storage::Storage;

/// The upscaler's `Storage` key, in the graphics namespace, and its key in the
/// project's `"graphics"` block.
pub const UPSCALER_KEY: &str = "upscaler";

/// A temporal upscaler: what resolves a TAA camera's jittered frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Upscaler {
    /// TAA's resolve, and TAAU's upscale below a render scale of 1 (#1702).
    #[default]
    Taa,
    /// AMD FidelityFX Super Resolution 2.2.1, ported to WGSL (#1848).
    Fsr2,
}

impl Upscaler {
    /// Every upscaler, in the order the docs list them.
    pub const ALL: [Self; 2] = [Self::Taa, Self::Fsr2];

    /// How the API and the project file spell it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Taa => "taa",
            Self::Fsr2 => "fsr2",
        }
    }

    /// The upscaler `name` spells; `Err` says what it was instead.
    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|u| u.name() == name)
            .ok_or_else(|| format!("'{name}' is not an upscaler (\"taa\" or \"fsr2\")"))
    }

    /// The saved upscaler, if the player chose one (a stored value that names
    /// none counts as none).
    pub fn load(storage: &Storage) -> Option<Self> {
        let value = storage.get(GRAPHICS_NAMESPACE, UPSCALER_KEY)?;
        Self::parse(value.as_str()?).ok()
    }

    /// Write the upscaler into `storage`.
    pub fn store(self, storage: &mut Storage) {
        let value = serde_json::Value::from(self.name());
        storage.set(GRAPHICS_NAMESPACE, UPSCALER_KEY, value);
    }
}

/// The upscaler the graphics cell carries: the player's choice over the project's
/// default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpscalerSetting {
    /// The player's choice (`Graphics.SetUpscaler`), saved; `None` runs the default.
    pub chosen: Option<Upscaler>,
    /// The project's default, as the run last read or set it.
    pub default: Upscaler,
}

impl UpscalerSetting {
    /// The upscaler that runs.
    pub fn resolve(self) -> Upscaler {
        self.chosen.unwrap_or(self.default)
    }

    /// The player's saved choice over `default`.
    pub fn load(storage: &Storage, default: Upscaler) -> Self {
        let chosen = Upscaler::load(storage);
        Self { chosen, default }
    }

    /// Write the choice into `storage`; no choice deletes the key, so the next
    /// launch runs the default too.
    pub fn store(self, storage: &mut Storage) {
        match self.chosen {
            Some(upscaler) => upscaler.store(storage),
            None => {
                storage.delete(GRAPHICS_NAMESPACE, UPSCALER_KEY);
            }
        }
    }
}

#[cfg(test)]
#[path = "upscaler_tests.rs"]
mod tests;
