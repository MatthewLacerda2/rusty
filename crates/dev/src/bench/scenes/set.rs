//! `--set key=value` (`make bench-scenes SET=…`, `make bench-ab SET=…`): a field
//! of the running quality preset overridden for one run, so an on/off comparison
//! (`SET=gpu_occlusion_culling=true`) needs no edit of the project.
//!
//! The override goes through the path `Graphics.SetQualityPreset(name, { … })`
//! takes (`QualityPresets::edited`), onto the preset the bench plays (the
//! project's initial one: a harness has no saved player choice), in memory: the
//! project's `project.rusty` is never written. A key is a field of
//! [`QualityValues`] as `project.rusty` names it, a nested one dotted
//! (`ssao.high.radius`); the value is read as JSON (`true`, `2`, `"High"`) and
//! as a plain string when it is not JSON (`DistanceShadowmask`). A key no
//! preset has fails before anything runs, listing the keys there are.

use rusty_core::settings::quality::{QualityPresets, QualityValues};
use serde_json::{Map, Value};

/// The run's overrides, in the order given.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    /// Each as given (`key=value`): what the report records and a child is passed.
    pub said: Vec<String>,
    /// Every override as one partial preset.
    patch: Map<String, Value>,
}

impl Overrides {
    /// Read `pairs` (`key=value` each), refusing a malformed pair or an unknown
    /// key (naming the keys a preset has) and a value of the wrong shape.
    pub fn parse(pairs: &[String]) -> Result<Self, String> {
        let mut out = Self::default();
        for pair in pairs {
            let Some((key, value)) = pair.split_once('=') else {
                return Err(format!("--set wants key=value, not '{pair}'"));
            };
            let value = serde_json::from_str(value).unwrap_or(Value::String(value.into()));
            insert(&mut out.patch, key.trim(), value)?;
            out.said.push(pair.clone());
        }
        // The shape is checked once, over the engine's values, before any run.
        QualityPresets::default().edited("Medium", &out.patch())?;
        Ok(out)
    }

    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }

    /// The overrides as the partial preset `Graphics.SetQualityPreset` takes.
    pub fn patch(&self) -> Value {
        Value::Object(self.patch.clone())
    }

    /// Write the overrides over the preset `presets` runs (its initial one; the
    /// engine's values, named `Medium`, for a project with none).
    pub fn apply(&self, presets: &mut QualityPresets) -> Result<(), String> {
        if self.is_empty() {
            return Ok(());
        }
        let name = presets
            .initial()
            .map_or("Medium".into(), |p| p.name.clone());
        let edited = presets.edited(&name, &self.patch())?;
        presets.set(edited);
        Ok(())
    }
}

/// Put `value` at the dotted `key` of `patch`, refusing a key no preset has.
fn insert(patch: &mut Map<String, Value>, key: &str, value: Value) -> Result<(), String> {
    let engine = serde_json::to_value(QualityValues::default()).map_err(|e| e.to_string())?;
    let mut known = &engine;
    let mut at = patch;
    let parts: Vec<&str> = key.split('.').collect();
    for (i, part) in parts.iter().enumerate() {
        known = known.get(part).ok_or_else(|| unknown(key, known))?;
        if i + 1 == parts.len() {
            at.insert(part.to_string(), value);
            return Ok(());
        }
        let slot = at
            .entry(part.to_string())
            .or_insert(Value::Object(Map::new()));
        at = slot
            .as_object_mut()
            .ok_or_else(|| format!("--set {key}: '{part}' is set twice, whole and in part"))?;
    }
    Err("--set wants a key before '='".into())
}

/// The refusal of `key`, listing the keys `level` (where it went wrong) has.
fn unknown(key: &str, level: &Value) -> String {
    let keys = level
        .as_object()
        .map(|o| o.keys().cloned().collect::<Vec<_>>());
    let keys = keys.unwrap_or_default().join(", ");
    format!("--set: a quality preset has no '{key}' (its keys there: {keys})")
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod set_tests;
