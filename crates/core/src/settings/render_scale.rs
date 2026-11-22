use serde::de::Deserializer;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use super::GRAPHICS_NAMESPACE;
use crate::project::PROJECT_FILE;
use crate::storage::Storage;

pub const RENDER_SCALE_KEY: &str = "render_scale";
pub const MATCH_DISPLAY: &str = "match_display";

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
    pub const MIN: f32 = 0.1;
    pub const MAX: f32 = 2.0;
    pub const NATIVE: Self = Self(1.0);

    /// `scale` clamped to the range; a NaN is the native scale.
    pub fn new(scale: f32) -> Self {
        match scale.is_nan() {
            true => Self::NATIVE,
            false => Self(scale.clamp(Self::MIN, Self::MAX)),
        }
    }

    pub fn get(self) -> f32 {
        self.0
    }

    pub fn apply(self, size: u32) -> u32 {
        ((size as f32 * self.0).round() as u32).max(1)
    }

    pub fn load(storage: &Storage) -> Option<Self> {
        let value = storage.get(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY)?;
        value.as_f64().map(|v| Self::new(v as f32))
    }

    pub fn store(self, storage: &mut Storage) {
        let value = serde_json::Value::from(f64::from(self.0));
        storage.set(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY, value);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderScaleDefault {
    Fixed(RenderScale),
    MatchDisplay,
}

impl Default for RenderScaleDefault {
    fn default() -> Self {
        Self::Fixed(RenderScale::NATIVE)
    }
}

impl RenderScaleDefault {
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

impl Serialize for RenderScaleDefault {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Fixed(scale) => serializer.serialize_f32(scale.get()),
            Self::MatchDisplay => serializer.serialize_str(MATCH_DISPLAY),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderScaleSetting {
    pub chosen: Option<RenderScale>,
    pub default: RenderScaleDefault,
}

impl RenderScaleSetting {
    pub fn resolve(self, display_scale: f32) -> RenderScale {
        (self.chosen).unwrap_or_else(|| self.default.resolve(display_scale))
    }

    pub fn load(storage: &Storage, default: RenderScaleDefault) -> Self {
        let chosen = RenderScale::load(storage);
        Self { chosen, default }
    }

    pub fn store(self, storage: &mut Storage) {
        match self.chosen {
            Some(scale) => scale.store(storage),
            None => {
                storage.delete(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ProjectGraphics {
    pub render_scale: RenderScaleDefault,
}

impl ProjectGraphics {
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
