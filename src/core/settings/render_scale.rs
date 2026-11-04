//! src/core/settings/render_scale.rs — `RenderScale`: the scene's render resolution
//! as a factor of the surface's (#1498), and where it comes from (#1715).
//!
//! Unity URP's *Render Scale* (`UniversalRenderPipelineAsset.renderScale`): every
//! scene render target of the screen's camera stack (colour, depth, the SSAO prepass,
//! motion vectors, the post-FX targets) is sized `surface × scale`, and the finished,
//! post-processed image is upscaled to the surface with bilinear filtering before the
//! UI, which stays at native resolution. On a 2× Retina surface, 0.5 renders the
//! scene at the 1× pixel count. Render-only: never read by `FixedUpdate`.
//!
//! The scale that runs is a [`RenderScaleSetting`]: the player's choice, persisted
//! beside the quality tier (`Storage` `graphics.render_scale`), over the project's
//! default ([`RenderScaleDefault`], `project.rusty`'s `"graphics"` block,
//! [`ProjectGraphics`]). The default is a factor or `"match_display"`: `1 / display
//! scale`, the 1× pixel density on a HiDPI display, following a monitor change. With
//! no choice stored the default runs, so forgetting the choice
//! (`Graphics.ResetRenderScale`) brings the default back.

use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use super::GRAPHICS_NAMESPACE;
use crate::core::project::PROJECT_FILE;
use crate::core::storage::Storage;

/// The render scale's `Storage` key, in the graphics namespace.
pub const RENDER_SCALE_KEY: &str = "render_scale";
/// How `project.rusty` and `Graphics.SetDefaultRenderScale` spell
/// [`RenderScaleDefault::MatchDisplay`].
pub const MATCH_DISPLAY: &str = "match_display";

/// A render-scale factor, always inside [`RenderScale::MIN`]..=[`RenderScale::MAX`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderScale(f32);

// A `RenderScale` is never NaN (`new` maps it to 1), so equality is total.
impl Eq for RenderScale {}

impl Default for RenderScale {
    fn default() -> Self {
        Self::NATIVE
    }
}

impl RenderScale {
    /// The smallest factor, URP's.
    pub const MIN: f32 = 0.1;
    /// The largest factor (supersampling), URP's.
    pub const MAX: f32 = 2.0;
    /// The surface's own resolution: no scaling and no upscale pass.
    pub const NATIVE: Self = Self(1.0);

    /// `scale` clamped to the range; a NaN is the native scale.
    pub fn new(scale: f32) -> Self {
        match scale.is_nan() {
            true => Self::NATIVE,
            false => Self(scale.clamp(Self::MIN, Self::MAX)),
        }
    }

    /// The factor.
    pub fn get(self) -> f32 {
        self.0
    }

    /// `size` (a surface's width or height in pixels) at this scale, at least 1.
    pub fn apply(self, size: u32) -> u32 {
        ((size as f32 * self.0).round() as u32).max(1)
    }

    /// The saved scale, if the player chose one (a stored value that is not a
    /// number counts as none).
    pub fn load(storage: &Storage) -> Option<Self> {
        let value = storage.get(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY)?;
        value.as_f64().map(|v| Self::new(v as f32))
    }

    /// Write the scale into `storage`.
    pub fn store(self, storage: &mut Storage) {
        let value = serde_json::Value::from(f64::from(self.0));
        storage.set(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY, value);
    }
}

/// The project's render scale (#1715): what runs while the player has chosen none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderScaleDefault {
    /// This factor, whatever the display.
    Fixed(RenderScale),
    /// `1 / display scale`: the scene at the 1× pixel count on a HiDPI display,
    /// native on a 1× one.
    MatchDisplay,
}

impl Default for RenderScaleDefault {
    fn default() -> Self {
        Self::Fixed(RenderScale::NATIVE)
    }
}

impl RenderScaleDefault {
    /// The factor on a display of `display_scale` (physical pixels per point). A
    /// scale that is not a positive number is taken as 1.
    pub fn resolve(self, display_scale: f32) -> RenderScale {
        match self {
            Self::Fixed(scale) => scale,
            Self::MatchDisplay if display_scale > 0.0 => RenderScale::new(1.0 / display_scale),
            Self::MatchDisplay => RenderScale::NATIVE,
        }
    }

    /// `value` as the project file spells a default: a number (clamped as
    /// [`RenderScale::new`]) or `"match_display"`; `Err` says what it was instead.
    pub fn read_json(value: &Value) -> Result<Self, String> {
        match value {
            Value::String(s) if s == MATCH_DISPLAY => Ok(Self::MatchDisplay),
            v => match v.as_f64().map(|v| v as f32) {
                Some(f) if f.is_finite() && f > 0.0 => Ok(Self::Fixed(RenderScale::new(f))),
                _ => Err(format!(
                    "{value} is not a positive number or \"{MATCH_DISPLAY}\""
                )),
            },
        }
    }
}

/// The factor as an `f32`, so `0.7` is written `0.7` (an `f64` widening would write
/// `0.699999988079071` into a hand-edited file), or `"match_display"`.
impl Serialize for RenderScaleDefault {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Fixed(scale) => serializer.serialize_f32(scale.get()),
            Self::MatchDisplay => serializer.serialize_str(MATCH_DISPLAY),
        }
    }
}

/// The render scale the graphics cell carries: the player's choice over the
/// project's default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderScaleSetting {
    /// The player's choice (`Graphics.SetRenderScale`), saved; `None` runs the default.
    pub chosen: Option<RenderScale>,
    /// The project's default, as the run last read or set it.
    pub default: RenderScaleDefault,
}

impl RenderScaleSetting {
    /// The factor that runs on a display of `display_scale`.
    pub fn resolve(self, display_scale: f32) -> RenderScale {
        (self.chosen).unwrap_or_else(|| self.default.resolve(display_scale))
    }

    /// The player's saved choice over `default`.
    pub fn load(storage: &Storage, default: RenderScaleDefault) -> Self {
        let chosen = RenderScale::load(storage);
        Self { chosen, default }
    }

    /// Write the choice into `storage`; no choice deletes the key, so the next
    /// launch runs the default too.
    pub fn store(self, storage: &mut Storage) {
        match self.chosen {
            Some(scale) => scale.store(storage),
            None => {
                storage.delete(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY);
            }
        }
    }
}

/// `project.rusty`'s `"graphics"` block (#1715): the project's render-scale default.
/// A missing or bad value keeps native, with a warning ([`ProjectGraphics::read_json`]),
/// so the project still opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ProjectGraphics {
    pub render_scale: RenderScaleDefault,
}

impl ProjectGraphics {
    /// The block `blob` spells, over the defaults, and a line for each part of it
    /// that was skipped (an unknown key, a value that is not a default).
    pub fn read_json(blob: &Value) -> (Self, Vec<String>) {
        let mut settings = Self::default();
        let Some(map) = blob.as_object() else {
            return (settings, vec![format!("expected an object, found {blob}")]);
        };
        let mut ignored = Vec::new();
        for (key, value) in map {
            if key != RENDER_SCALE_KEY {
                ignored.push(format!("unknown key '{key}' (one of {RENDER_SCALE_KEY})"));
                continue;
            }
            match RenderScaleDefault::read_json(value) {
                Ok(default) => settings.render_scale = default,
                Err(why) => ignored.push(format!("{RENDER_SCALE_KEY}: {why}; using 1")),
            }
        }
        (settings, ignored)
    }
}

impl<'de> Deserialize<'de> for ProjectGraphics {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (settings, ignored) = Self::read_json(&Value::deserialize(deserializer)?);
        for line in ignored {
            log::warn!("[Project] {PROJECT_FILE} graphics: {line}");
        }
        Ok(settings)
    }
}

#[cfg(test)]
#[path = "render_scale_tests.rs"]
mod tests;
