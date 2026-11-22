use rusty_core::project::ProjectSettings;
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_core::settings::quality::{QualityPresets, QualityValues};
use rusty_render::{EarlyFlushes, RenderSwitch, RenderSwitches};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    pub said: Vec<String>,
    patch: Map<String, Value>,
    options: Vec<(GraphicsOption, bool)>,
    switches: RenderSwitches,
}

impl Overrides {
    pub fn parse(pairs: &[String]) -> Result<Self, String> {
        let mut out = Self::default();
        for pair in pairs {
            let Some((key, value)) = pair.split_once('=') else {
                return Err(format!("--set wants key=value, not '{pair}'"));
            };
            let key = key.trim();
            match key.split_once('.') {
                Some(("option", name)) => {
                    let option = GraphicsOption::parse(name).ok_or_else(|| unknown_in(key))?;
                    out.options.push((option, flag(key, value)?));
                }
                Some(("render", "flushes")) => {
                    let flushes = EarlyFlushes::parse(value.trim()).ok_or_else(|| {
                        format!(
                            "--set render.flushes wants {}, not '{value}'",
                            EarlyFlushes::names()
                        )
                    })?;
                    out.switches.set_flushes(flushes);
                }
                Some(("render", name)) => {
                    let switch = RenderSwitch::parse(name).ok_or_else(|| unknown_in(key))?;
                    out.switches.set(switch, flag(key, value)?);
                }
                _ => {
                    let value = serde_json::from_str(value).unwrap_or(Value::String(value.into()));
                    insert(&mut out.patch, key, value)?;
                }
            }
            out.said.push(pair.clone());
        }
        QualityPresets::default().edited("Medium", &out.patch())?;
        Ok(out)
    }

    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }

    pub fn patch(&self) -> Value {
        Value::Object(self.patch.clone())
    }

    pub fn switches(&self) -> RenderSwitches {
        self.switches
    }

    pub fn apply_settings(&self, settings: &mut ProjectSettings) -> Result<(), String> {
        self.apply_options(&mut settings.graphics_options);
        self.apply(&mut settings.quality)
    }

    pub fn apply_options(&self, options: &mut GraphicsOptions) {
        for &(option, on) in &self.options {
            options.set(option, on);
        }
    }

    pub fn apply(&self, presets: &mut QualityPresets) -> Result<(), String> {
        if self.patch.is_empty() {
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

fn flag(key: &str, value: &str) -> Result<bool, String> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("--set {key} wants true or false, not '{other}'")),
    }
}

fn unknown_in(key: &str) -> String {
    format!("--set: no '{key}' ({})", namespaces())
}

fn namespaces() -> String {
    format!(
        "option.<name>: {}; render.<switch>: {}; render.flushes: {}",
        GraphicsOption::names(),
        RenderSwitch::names(),
        EarlyFlushes::names()
    )
}

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

fn unknown(key: &str, level: &Value) -> String {
    let keys = level
        .as_object()
        .map(|o| o.keys().cloned().collect::<Vec<_>>());
    let keys = keys.unwrap_or_default().join(", ");
    format!(
        "--set: a quality preset has no '{key}' (its keys there: {keys}; or {})",
        namespaces()
    )
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod set_tests;
