//! `bench-scenes --set key=value` (`make bench-ab SET=…`): one input of the run
//! overridden for that run alone, so an on/off comparison
//! (`SET=gpu_occlusion_culling=true`, `SET=render.shadows=false`) needs no edit
//! of the project. A key names its source by its namespace:
//!
//! - **a field of the running quality preset**, as `project.rusty` names it, a
//!   nested one dotted (`ssao.high.radius`): written through the path
//!   `Graphics.SetQualityPreset(name, { … })` takes (`QualityPresets::edited`),
//!   onto the preset the bench plays (the project's initial one: a harness has no
//!   saved player choice). The value is read as JSON (`true`, `2`, `"High"`) and
//!   as a plain string when it is not JSON (`DistanceShadowmask`);
//! - **`option.<name>`, a player graphics option** as `Graphics.SetOption` names
//!   it (`option.ssao=false`), written over the project's defaults, which the run
//!   plays with no saved player choice. It reaches the game rounds only: the
//!   editor round's Scene view ignores the player's options, as Unity's does;
//! - **`render.<switch>`, a renderer measurement switch**
//!   (`rusty_render::RenderSwitch`: `render.shadows=false`, `render.lightmaps`,
//!   `render.reflection_probes`, `render.clustered_lights`, `render.ssao`,
//!   `render.ssil`), handed to every renderer the run builds, the editor round's
//!   too, through `Options::switches`: what the forward pass reads is bound to its
//!   stand-in, no pipeline or layout changing (`render::stats::switches` says what
//!   each takes and what stays).
//!
//! Everything is in memory: the project's `project.rusty` is never written. A
//! key no namespace has fails before anything runs, listing every namespace's
//! keys; an option or a switch takes `true` or `false`.

use rusty_core::project::ProjectSettings;
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_core::settings::quality::{QualityPresets, QualityValues};
use rusty_render::{RenderSwitch, RenderSwitches};
use serde_json::{Map, Value};

/// The run's overrides, in the order given.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    /// Each as given (`key=value`): what the report records and a child is passed.
    pub said: Vec<String>,
    /// Every quality override as one partial preset.
    patch: Map<String, Value>,
    /// The `option.` overrides, in the order given.
    options: Vec<(GraphicsOption, bool)>,
    /// The `render.` switches.
    switches: RenderSwitches,
}

impl Overrides {
    /// Read `pairs` (`key=value` each), refusing a malformed pair or an unknown
    /// key (naming every namespace's keys) and a value of the wrong shape.
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
        // The shape is checked once, over the engine's values, before any run.
        QualityPresets::default().edited("Medium", &out.patch())?;
        Ok(out)
    }

    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }

    /// The quality overrides as the partial preset `Graphics.SetQualityPreset` takes.
    pub fn patch(&self) -> Value {
        Value::Object(self.patch.clone())
    }

    /// The renderer switches the run draws with (all on unless a `render.` key
    /// turned one off): `Options::switches`.
    pub fn switches(&self) -> RenderSwitches {
        self.switches
    }

    /// Write the quality and option overrides over `settings`: the preset it runs
    /// ([`Self::apply`]) and its graphics option defaults ([`Self::apply_options`]).
    pub fn apply_settings(&self, settings: &mut ProjectSettings) -> Result<(), String> {
        self.apply_options(&mut settings.graphics_options);
        self.apply(&mut settings.quality)
    }

    /// Write the `option.` overrides over the project's option defaults.
    pub fn apply_options(&self, options: &mut GraphicsOptions) {
        for &(option, on) in &self.options {
            options.set(option, on);
        }
    }

    /// Write the quality overrides over the preset `presets` runs (its initial
    /// one; the engine's values, named `Medium`, for a project with none).
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

/// The `true` or `false` an option or a switch `key` takes.
fn flag(key: &str, value: &str) -> Result<bool, String> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("--set {key} wants true or false, not '{other}'")),
    }
}

/// The refusal of an `option.` or `render.` key that names nothing.
fn unknown_in(key: &str) -> String {
    format!("--set: no '{key}' ({})", namespaces())
}

/// The keys of the two flat namespaces, for every refusal of an unknown key.
fn namespaces() -> String {
    format!(
        "option.<name>: {}; render.<switch>: {}",
        GraphicsOption::names(),
        RenderSwitch::names()
    )
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
    format!(
        "--set: a quality preset has no '{key}' (its keys there: {keys}; or {})",
        namespaces()
    )
}

#[cfg(test)]
#[path = "set_tests.rs"]
mod set_tests;
