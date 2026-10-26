//! src/core/settings/graphics_options.rs — `GraphicsOptions`: the player's
//! per-effect switches (#1438).
//!
//! What an options menu's "Motion blur: off" writes. An effect runs for a view only
//! when its camera has it on, the player option allows it **and** the quality tier
//! runs it (`render::postfx::params::effect_runs`), so the developer's authored look
//! stays on the camera and the player's choice holds for every camera and scene.
//!
//! Three layers, Unity's PlayerPrefs over Player Settings:
//!
//! * **The project defaults** — what a fresh install starts with, in the tracked
//!   `project.rusty` under `"graphics_options"` (`core::project::ProjectFile`),
//!   held by the `Application` resource beside the build settings.
//! * **The player's choice** — the live value `Graphics.SetOption` writes, persisted
//!   to `Storage` namespace `graphics`, key `options` ([`GraphicsOptions::load`] /
//!   [`GraphicsOptions::store`]), beside the quality tier.
//! * **The camera's look** — the camera's own component, which the options only gate.
//!
//! Render-only: never read by `FixedUpdate`, so the sim stays a function of its
//! inputs. A new player-facing effect is one [`GraphicsOption`] row.

use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::core::storage::Storage;

crate::core::named_enum! {
    /// One player-facing effect switch, named as `Graphics.SetOption` spells it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum GraphicsOption {
        /// Camera motion blur (also off on the Low tier).
        MotionBlur = "motion_blur",
        /// FXAA anti-aliasing.
        Fxaa = "fxaa",
        /// Bloom.
        Bloom = "bloom",
        /// Screen-space reflections (High tier only).
        Ssr = "ssr",
        /// Screen-space ambient occlusion (off on the Low tier).
        Ssao = "ssao",
    }
}

/// `Storage` namespace + key the player's options persist under (the quality tier's
/// namespace, key `options`).
pub const OPTIONS_NAMESPACE: &str = "graphics";
pub const OPTIONS_KEY: &str = "options";

/// One switch per [`GraphicsOption`], every one allowed by default. Stored as a JSON
/// object of name → bool; a name it omits keeps the fallback it is read over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicsOptions {
    allowed: [bool; GraphicsOption::ALL.len()],
}

impl Default for GraphicsOptions {
    fn default() -> Self {
        Self {
            allowed: [true; GraphicsOption::ALL.len()],
        }
    }
}

impl GraphicsOptions {
    /// Whether the player allows `option`.
    pub fn allows(&self, option: GraphicsOption) -> bool {
        self.allowed[option as usize]
    }

    /// Allow or forbid `option`.
    pub fn set(&mut self, option: GraphicsOption, allowed: bool) {
        self.allowed[option as usize] = allowed;
    }

    /// The JSON object `{ "motion_blur": true, … }`, every option named.
    pub fn to_json(&self) -> Value {
        let map = GraphicsOption::ALL
            .iter()
            .map(|&o| (o.name().to_string(), Value::Bool(self.allows(o))))
            .collect::<Map<_, _>>();
        Value::Object(map)
    }

    /// `fallback` with every option `blob` names as a bool overriding it. A blob
    /// that isn't an object, an unknown name or a non-bool value changes nothing.
    pub fn from_json(blob: &Value, fallback: Self) -> Self {
        let mut options = fallback;
        for (name, value) in blob.as_object().into_iter().flatten() {
            if let (Some(option), Some(on)) = (GraphicsOption::parse(name), value.as_bool()) {
                options.set(option, on);
            }
        }
        options
    }

    /// The player's saved options over `defaults` (the project's): what startup
    /// reads. A missing or partial blob keeps the defaults for what it omits.
    pub fn load(storage: &Storage, defaults: Self) -> Self {
        storage
            .get(OPTIONS_NAMESPACE, OPTIONS_KEY)
            .map_or(defaults, |blob| Self::from_json(&blob, defaults))
    }

    /// Write these options into `storage` (the Stop / quit flush puts them on disk).
    pub fn store(&self, storage: &mut Storage) {
        storage.set(OPTIONS_NAMESPACE, OPTIONS_KEY, self.to_json());
    }
}

/// The `project.rusty` spelling: the same object, read over the all-allowed default.
impl Serialize for GraphicsOptions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraphicsOptions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let blob = Value::deserialize(deserializer)?;
        Ok(Self::from_json(&blob, Self::default()))
    }
}

#[cfg(test)]
#[path = "graphics_options_tests.rs"]
mod tests;
