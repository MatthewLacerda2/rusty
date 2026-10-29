//! The game's quality presets, as `project.rusty` holds them (#1522): an ordered,
//! named list, the one a first launch selects, and the edits the `Graphics` API and
//! the editor make to it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{seeds, QualityValues};

/// One named preset: what a player's Low / Medium / High menu entry selects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityPreset {
    pub name: String,
    #[serde(flatten)]
    pub values: QualityValues,
}

/// The game's quality presets, in order, and the one a first launch selects.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityPresets {
    /// The preset a player with no saved choice gets; `None` (or a name no preset
    /// has) selects the middle one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub presets: Vec<QualityPreset>,
}

impl Default for QualityPresets {
    /// What a new project is seeded with: `Low`, `Medium`, `High`, `Medium` first.
    fn default() -> Self {
        let preset = |name: &str, values| QualityPreset {
            name: name.to_string(),
            values,
        };
        Self {
            default: Some("Medium".to_string()),
            presets: vec![
                preset("Low", seeds::low()),
                preset("Medium", seeds::medium()),
                preset("High", seeds::high()),
            ],
        }
    }
}

impl QualityPresets {
    /// The preset called `name`, ignoring ASCII case.
    pub fn get(&self, name: &str) -> Option<&QualityPreset> {
        self.presets
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// The presets' names, in order.
    pub fn names(&self) -> Vec<&str> {
        self.presets.iter().map(|p| p.name.as_str()).collect()
    }

    /// The preset a player with no saved choice gets: [`default`](Self::default)'s,
    /// else the middle one; `None` when the game defines none.
    pub fn initial(&self) -> Option<&QualityPreset> {
        let named = self.default.as_deref().and_then(|n| self.get(n));
        named.or_else(|| self.presets.get(self.presets.len() / 2))
    }

    /// The preset to run when `selected` is the player's choice: it, when it still
    /// exists, else [`initial`](Self::initial).
    pub fn resolve(&self, selected: Option<&str>) -> Option<&QualityPreset> {
        selected
            .and_then(|n| self.get(n))
            .or_else(|| self.initial())
    }

    /// The values `selected` runs with: its preset's, else the engine's own when
    /// the game defines no preset at all.
    pub fn values(&self, selected: Option<&str>) -> QualityValues {
        self.resolve(selected).map(|p| p.values).unwrap_or_default()
    }

    /// Add `preset`, or replace the one of its name (ignoring ASCII case) in place.
    pub fn set(&mut self, preset: QualityPreset) {
        let same = |p: &&mut QualityPreset| p.name.eq_ignore_ascii_case(&preset.name);
        match self.presets.iter_mut().find(same) {
            Some(slot) => *slot = preset,
            None => self.presets.push(preset),
        }
    }

    /// Remove the preset called `name`; `false` when there is none.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| !p.name.eq_ignore_ascii_case(name));
        self.presets.len() != before
    }

    /// The preset `name` becomes when `patch` (a JSON object shaped like
    /// [`QualityValues`], any part of it) is written over it: over the preset of
    /// that name, else over the engine's own values for a new one. A key the values
    /// don't have, or a value of the wrong shape, is refused.
    pub fn edited(&self, name: &str, patch: &Value) -> Result<QualityPreset, String> {
        let base = self.get(name).map(|p| p.values).unwrap_or_default();
        let mut merged = serde_json::to_value(base).map_err(|e| e.to_string())?;
        merge(&mut merged, patch, "")?;
        let values = serde_json::from_value(merged).map_err(|e| e.to_string())?;
        Ok(QualityPreset {
            name: name.to_string(),
            values,
        })
    }

    /// The refusal for a name no preset has, listing the ones there are.
    pub fn unknown(&self, name: &str) -> String {
        format!(
            "unknown quality preset '{name}' (the project has: {})",
            self.names().join(", ")
        )
    }
}

/// Write `patch` over `base` key by key, `path` naming where for a refusal.
fn merge(base: &mut Value, patch: &Value, path: &str) -> Result<(), String> {
    let (Value::Object(base), Value::Object(patch)) = (&mut *base, patch) else {
        *base = patch.clone();
        return Ok(());
    };
    for (key, value) in patch {
        let at = format!("{path}{key}");
        let slot = base
            .get_mut(key)
            .ok_or_else(|| format!("a quality preset has no '{at}'"))?;
        merge(slot, value, &format!("{at}."))?;
    }
    Ok(())
}
