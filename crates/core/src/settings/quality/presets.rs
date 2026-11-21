use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{partial, seeds, QualityValues};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityPreset {
    pub name: String,
    #[serde(flatten)]
    pub values: QualityValues,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QualityPresets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    pub presets: Vec<QualityPreset>,
}

impl Default for QualityPresets {
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
    pub fn get(&self, name: &str) -> Option<&QualityPreset> {
        self.presets
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn names(&self) -> Vec<&str> {
        self.presets.iter().map(|p| p.name.as_str()).collect()
    }

    pub fn initial(&self) -> Option<&QualityPreset> {
        let named = self.default.as_deref().and_then(|n| self.get(n));
        named.or_else(|| self.presets.get(self.presets.len() / 2))
    }

    pub fn resolve(&self, selected: Option<&str>) -> Option<&QualityPreset> {
        selected
            .and_then(|n| self.get(n))
            .or_else(|| self.initial())
    }

    pub fn values(&self, selected: Option<&str>) -> QualityValues {
        self.resolve(selected).map(|p| p.values).unwrap_or_default()
    }

    pub fn set(&mut self, preset: QualityPreset) {
        let same = |p: &&mut QualityPreset| p.name.eq_ignore_ascii_case(&preset.name);
        match self.presets.iter_mut().find(same) {
            Some(slot) => *slot = preset,
            None => self.presets.push(preset),
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| !p.name.eq_ignore_ascii_case(name));
        self.presets.len() != before
    }

    pub fn edited(&self, name: &str, patch: &Value) -> Result<QualityPreset, String> {
        let mut patch = patch.clone();
        let named = patch.as_object_mut().and_then(|o| o.remove("name"));
        if let Some(named) =
            named.filter(|n| !n.as_str().is_some_and(|n| n.eq_ignore_ascii_case(name)))
        {
            return Err(format!("the values are named {named}, not '{name}'"));
        }
        let base = self.get(name).map(|p| p.values).unwrap_or_default();
        let mut merged = serde_json::to_value(base).map_err(|e| e.to_string())?;
        partial::merge(&mut merged, &patch, "")?;
        let values = serde_json::from_value(merged).map_err(|e| e.to_string())?;
        Ok(QualityPreset {
            name: name.to_string(),
            values,
        })
    }

    pub fn unknown(&self, name: &str) -> String {
        format!(
            "unknown quality preset '{name}' (the project has: {})",
            self.names().join(", ")
        )
    }
}
