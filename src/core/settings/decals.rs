//! src/core/settings/decals.rs — `DecalLimits`: the per-game decal budgets (#1517).
//!
//! How many decals a game keeps whole at once, in three buckets: per owner entity
//! (a bone is its own owner), the world (decals stamped with no owner) and the
//! scene's total, a performance ceiling. Past a budget the oldest decal of that
//! bucket fades out (`scene::decal::DecalSet`).
//!
//! The project's values live in the tracked `project.rusty` under `"decals"`
//! (`core::project::ProjectFile`) and are applied to the scene on entering Play;
//! `Decals.SetLimits` changes the running game's until Stop.

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::project::PROJECT_FILE;

/// The `project.rusty` keys, in the order a warning lists them.
const KEYS: &str = "max_per_owner, max_world, max_total";

/// The decal budgets. A partial `"decals"` block keeps the defaults for what it
/// omits; an unknown key or a value that is not a whole number is skipped with a
/// warning ([`DecalLimits::read_json`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DecalLimits {
    /// Whole decals one owner entity keeps.
    pub max_per_owner: u32,
    /// Whole decals with no owner (world space), as one bucket.
    pub max_world: u32,
    /// Whole decals in the scene, every bucket together.
    pub max_total: u32,
}

impl Default for DecalLimits {
    fn default() -> Self {
        Self {
            max_per_owner: 256,
            max_world: 1024,
            max_total: 4096,
        }
    }
}

impl DecalLimits {
    /// The budgets `blob` spells, over the defaults, and a line for each part of it
    /// that was skipped (an unknown key, a value that is not a whole number).
    pub fn read_json(blob: &Value) -> (Self, Vec<String>) {
        let mut limits = Self::default();
        let Some(map) = blob.as_object() else {
            return (limits, vec![format!("expected an object, found {blob}")]);
        };
        let mut ignored = Vec::new();
        for (key, value) in map {
            let slot = match key.as_str() {
                "max_per_owner" => &mut limits.max_per_owner,
                "max_world" => &mut limits.max_world,
                "max_total" => &mut limits.max_total,
                _ => {
                    ignored.push(format!("unknown key '{key}' (one of {KEYS})"));
                    continue;
                }
            };
            match value.as_u64().and_then(|n| u32::try_from(n).ok()) {
                Some(n) => *slot = n,
                None => ignored.push(format!("'{key}' is {value}, not a whole number")),
            }
        }
        (limits, ignored)
    }
}

impl<'de> Deserialize<'de> for DecalLimits {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (limits, ignored) = Self::read_json(&Value::deserialize(deserializer)?);
        // A typo is the developer's to fix, so say so, as `graphics_options` does
        // (a logged warning reaches the console and the MCP).
        for line in ignored {
            log::warn!("[Project] {PROJECT_FILE} decals: {line}");
        }
        Ok(limits)
    }
}

#[cfg(test)]
#[path = "decals_tests.rs"]
mod tests;
