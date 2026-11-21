use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::PROJECT_FILE;

const KEYS: &str = "max_per_owner, max_world, max_total";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DecalLimits {
    pub max_per_owner: u32,
    pub max_world: u32,
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
        for line in ignored {
            log::warn!("[Project] {PROJECT_FILE} decals: {line}");
        }
        Ok(limits)
    }
}

#[cfg(test)]
#[path = "decals_tests.rs"]
mod tests;
