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
//!   [`GraphicsOptions::store`]), beside the quality tier. Only what the player set
//!   is saved, so a later change to a project default still reaches them.
//! * **The camera's look** — the camera's own component, which the options only gate.
//!
//! Render-only: never read by `FixedUpdate`, so the sim stays a function of its
//! inputs. A new player-facing effect is one [`GraphicsOption`] row.

use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::GRAPHICS_NAMESPACE;
use crate::core::project::PROJECT_FILE;
use crate::core::storage::Storage;

crate::core::named_enum! {
    /// One player-facing effect switch, named as `Graphics.SetOption` spells it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum GraphicsOption {
        // sorted: one row per player-facing effect; menus list them in this order
        /// Bloom.
        Bloom = "bloom",
        /// The volume's chromatic aberration (#1439).
        ChromaticAberration = "chromatic_aberration",
        /// The volume's depth of field (#1445; off on the Low tier).
        DepthOfField = "depth_of_field",
        /// The volume's film grain (#1439).
        FilmGrain = "film_grain",
        /// FXAA anti-aliasing.
        Fxaa = "fxaa",
        /// The volume's lens flare (#1443; off on the Low tier, and only while bloom runs).
        LensFlare = "lens_flare",
        /// Camera motion blur (also off on the Low tier).
        MotionBlur = "motion_blur",
        /// Screen-space ambient occlusion (off on the Low tier).
        Ssao = "ssao",
        /// Screen-space reflections (High tier only).
        Ssr = "ssr",
        /// The volume's vignette (#1439).
        Vignette = "vignette",
    }
}

/// `Storage` key the player's options persist under, in the graphics namespace
/// ([`GRAPHICS_NAMESPACE`]) beside the quality tier.
pub const OPTIONS_KEY: &str = "options";

/// One switch per [`GraphicsOption`], every one allowed by default, and which of them
/// the player has set (Unity's `PlayerPrefs.HasKey`). Stored as a JSON object of
/// name → bool; a name it omits keeps the fallback it is read over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicsOptions {
    allowed: [bool; GraphicsOption::ALL.len()],
    /// The options the player set: only these are saved, so every other one keeps
    /// following the project default, even one changed after the first launch.
    chosen: [bool; GraphicsOption::ALL.len()],
}

impl Default for GraphicsOptions {
    fn default() -> Self {
        Self {
            allowed: [true; GraphicsOption::ALL.len()],
            chosen: [false; GraphicsOption::ALL.len()],
        }
    }
}

impl GraphicsOptions {
    /// Whether the player allows `option`.
    pub fn allows(&self, option: GraphicsOption) -> bool {
        self.allowed[option as usize]
    }

    /// Allow or forbid `option`, as a default (not the player's choice).
    pub fn set(&mut self, option: GraphicsOption, allowed: bool) {
        self.allowed[option as usize] = allowed;
    }

    /// The player's choice for `option` (`Graphics.SetOption`): set, and saved.
    pub fn choose(&mut self, option: GraphicsOption, allowed: bool) {
        self.set(option, allowed);
        self.chosen[option as usize] = true;
    }

    /// Back to `defaults`, nothing chosen (`Graphics.ResetOptions`).
    pub fn reset(&mut self, defaults: Self) {
        *self = Self {
            allowed: defaults.allowed,
            ..Self::default()
        };
    }

    /// Follow new project `defaults` for every option the player hasn't set (a
    /// Build Settings toggle or `Graphics.SetDefaultOption`).
    pub fn rebase(&mut self, defaults: Self) {
        for option in GraphicsOption::ALL {
            if !self.chosen[option as usize] {
                self.set(option, defaults.allows(option));
            }
        }
    }

    /// The JSON object `{ "motion_blur": true, … }`, every option named (when
    /// `chosen_only`, only the player's choices).
    fn to_json(self, chosen_only: bool) -> Value {
        let map = GraphicsOption::ALL
            .iter()
            .filter(|&&o| !chosen_only || self.chosen[o as usize])
            .map(|&o| (o.name().to_string(), Value::Bool(self.allows(o))))
            .collect::<Map<_, _>>();
        Value::Object(map)
    }

    /// `fallback` with every option `blob` names as a bool overriding it, plus one
    /// line per entry it ignored (a blob that isn't an object, an unknown name, a
    /// non-bool value). `chosen` marks what the blob set as the player's.
    fn read_json(blob: &Value, fallback: Self, chosen: bool) -> (Self, Vec<String>) {
        let mut options = fallback;
        let Some(map) = blob.as_object() else {
            return (options, vec![format!("expected an object, found {blob}")]);
        };
        let mut ignored = Vec::new();
        for (name, value) in map {
            match (GraphicsOption::parse(name), value.as_bool()) {
                (Some(option), Some(on)) if chosen => options.choose(option, on),
                (Some(option), Some(on)) => options.set(option, on),
                (None, _) => ignored.push(format!(
                    "unknown option '{name}' (one of {})",
                    GraphicsOption::names()
                )),
                (Some(_), None) => ignored.push(format!("'{name}' is {value}, not a bool")),
            }
        }
        (options, ignored)
    }

    /// `fallback` with the player's choices `blob` holds over it. Lenient: junk is
    /// skipped silently, as a player's save is never theirs to fix by hand.
    pub fn from_json(blob: &Value, fallback: Self) -> Self {
        Self::read_json(blob, fallback, true).0
    }

    /// The player's saved options over `defaults` (the project's): what startup
    /// reads. A missing or partial blob keeps the defaults for what it omits.
    pub fn load(storage: &Storage, defaults: Self) -> Self {
        storage
            .get(GRAPHICS_NAMESPACE, OPTIONS_KEY)
            .map_or(defaults, |blob| Self::from_json(&blob, defaults))
    }

    /// Write the options the player set into `storage` (the Stop / quit flush puts
    /// them on disk); the rest stay unsaved, following the project default.
    pub fn store(&self, storage: &mut Storage) {
        storage.set(GRAPHICS_NAMESPACE, OPTIONS_KEY, self.to_json(true));
    }
}

/// The `project.rusty` spelling: every option by name, read over the all-allowed
/// default. Nothing in it is a player's choice.
impl Serialize for GraphicsOptions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_json(false).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraphicsOptions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let blob = Value::deserialize(deserializer)?;
        let (options, ignored) = Self::read_json(&blob, Self::default(), false);
        // The tracked file has no old builds to tolerate: a typo is the developer's
        // to fix, so say so (a logged warning reaches the console and the MCP).
        for line in ignored {
            log::warn!("[Project] {PROJECT_FILE} graphics_options: {line}");
        }
        Ok(options)
    }
}

#[cfg(test)]
#[path = "graphics_options_tests.rs"]
mod tests;
