//! src/core/settings/project_graphics.rs — `project.rusty`'s `"graphics"` block:
//! the project's defaults for what the player may choose in a graphics menu, the
//! render scale (#1715) and the upscaler (#1848).

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::render_scale::{RenderScaleDefault, RENDER_SCALE_KEY};
use super::upscaler::{Upscaler, UPSCALER_KEY};
use crate::core::project::PROJECT_FILE;

/// `project.rusty`'s `"graphics"` block: the project's render-scale and upscaler
/// defaults. A missing or bad value keeps its default (native, `"taa"`), with a
/// warning ([`ProjectGraphics::read_json`]), so the project still opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ProjectGraphics {
    pub render_scale: RenderScaleDefault,
    pub upscaler: Upscaler,
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
            match key.as_str() {
                RENDER_SCALE_KEY => match RenderScaleDefault::read_json(value) {
                    Ok(default) => settings.render_scale = default,
                    Err(why) => ignored.push(format!("{RENDER_SCALE_KEY}: {why}; using 1")),
                },
                UPSCALER_KEY => match value.as_str().map(Upscaler::parse) {
                    Some(Ok(upscaler)) => settings.upscaler = upscaler,
                    Some(Err(why)) => ignored.push(format!("{UPSCALER_KEY}: {why}; using taa")),
                    None => ignored.push(format!("{UPSCALER_KEY}: {value} is not a name")),
                },
                _ => ignored.push(format!(
                    "unknown key '{key}' (one of {RENDER_SCALE_KEY}, {UPSCALER_KEY})"
                )),
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
