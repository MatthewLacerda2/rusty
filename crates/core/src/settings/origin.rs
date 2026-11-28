//! The floating origin's project setting, off by default.

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::PROJECT_FILE;

/// Metres of one shift's lattice: a power of two (so subtracting it from a far `f32` is
/// exact) and a multiple of every baked lattice (the navmesh's 0.25 m columns and its
/// 32 m tiles, the PVS's cells), so shifted data keeps its alignment.
pub const ORIGIN_STEP: f32 = 1024.0;

/// Metres from zero past which the World shifts: a float still steps by half a millimetre.
pub const DEFAULT_THRESHOLD: f32 = 4096.0;

const KEYS: &str = "enabled, threshold";

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct OriginSettings {
    pub enabled: bool,
    pub threshold: f32,
}

impl Default for OriginSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: DEFAULT_THRESHOLD,
        }
    }
}

impl OriginSettings {
    /// Under one step, a threshold would shift back and forth.
    pub fn read_json(blob: &Value) -> (Self, Vec<String>) {
        let mut out = Self::default();
        let Some(map) = blob.as_object() else {
            return (out, vec![format!("expected an object, found {blob}")]);
        };
        let mut ignored = Vec::new();
        for (key, value) in map {
            match key.as_str() {
                "enabled" => match value.as_bool() {
                    Some(on) => out.enabled = on,
                    None => ignored.push(format!("'enabled' is {value}, not true or false")),
                },
                "threshold" => match value.as_f64().map(|t| t as f32) {
                    Some(t) if t.is_finite() && t >= ORIGIN_STEP => out.threshold = t,
                    _ => ignored.push(format!(
                        "'threshold' is {value}, not a number of metres of at least {ORIGIN_STEP}"
                    )),
                },
                _ => ignored.push(format!("unknown key '{key}' (one of {KEYS})")),
            }
        }
        (out, ignored)
    }
}

impl<'de> Deserialize<'de> for OriginSettings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (settings, ignored) = Self::read_json(&Value::deserialize(deserializer)?);
        for line in ignored {
            log::warn!("[Project] {PROJECT_FILE} floating_origin: {line}");
        }
        Ok(settings)
    }
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod origin_tests;
