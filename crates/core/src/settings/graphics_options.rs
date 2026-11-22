use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::GRAPHICS_NAMESPACE;
use crate::project::PROJECT_FILE;
use crate::storage::Storage;

crate::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum GraphicsOption {
        // sorted: one row per player-facing effect; menus list them in this order
        /// Bloom.
        Bloom = "bloom",
        ChromaticAberration = "chromatic_aberration",
        DepthOfField = "depth_of_field",
        FilmGrain = "film_grain",
        Fxaa = "fxaa",
        LensFlare = "lens_flare",
        MotionBlur = "motion_blur",
        Ssao = "ssao",
        Ssil = "ssil",
        Ssr = "ssr",
        Taa = "taa",
        Vignette = "vignette",
        VolumetricFog = "volumetric_fog",
    }
}

pub const OPTIONS_KEY: &str = "options";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphicsOptions {
    allowed: [bool; GraphicsOption::ALL.len()],
    /// What the player set: only these are saved, so the rest follow the project's
    /// defaults, even ones changed later.
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
    pub fn allows(&self, option: GraphicsOption) -> bool {
        self.allowed[option as usize]
    }

    /// As a default, not the player's choice: [`choose`](Self::choose) marks it saved.
    pub fn set(&mut self, option: GraphicsOption, allowed: bool) {
        self.allowed[option as usize] = allowed;
    }

    pub fn choose(&mut self, option: GraphicsOption, allowed: bool) {
        self.set(option, allowed);
        self.chosen[option as usize] = true;
    }

    pub fn reset(&mut self, defaults: Self) {
        *self = Self {
            allowed: defaults.allowed,
            ..Self::default()
        };
    }

    pub fn rebase(&mut self, defaults: Self) {
        for option in GraphicsOption::ALL {
            if !self.chosen[option as usize] {
                self.set(option, defaults.allows(option));
            }
        }
    }

    fn to_json(self, chosen_only: bool) -> Value {
        let map = GraphicsOption::ALL
            .iter()
            .filter(|&&o| !chosen_only || self.chosen[o as usize])
            .map(|&o| (o.name().to_string(), Value::Bool(self.allows(o))))
            .collect::<Map<_, _>>();
        Value::Object(map)
    }

    /// `chosen` marks what `blob` sets as the player's choice, so it is saved again.
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

    pub fn load(storage: &Storage, defaults: Self) -> Self {
        storage
            .get(GRAPHICS_NAMESPACE, OPTIONS_KEY)
            .map_or(defaults, |blob| Self::from_json(&blob, defaults))
    }

    pub fn store(&self, storage: &mut Storage) {
        storage.set(GRAPHICS_NAMESPACE, OPTIONS_KEY, self.to_json(true));
    }
}

impl Serialize for GraphicsOptions {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_json(false).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraphicsOptions {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let blob = Value::deserialize(deserializer)?;
        let (options, ignored) = Self::read_json(&blob, Self::default(), false);
        // The tracked file is the developer's to fix, so what it skips is warned about.
        for line in ignored {
            log::warn!("[Project] {PROJECT_FILE} graphics_options: {line}");
        }
        Ok(options)
    }
}

#[cfg(test)]
#[path = "graphics_options_tests.rs"]
mod tests;
