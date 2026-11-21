use serde_json::Value;

use super::Application;
use crate::settings::quality::{QualityPresets, QualityValues};

impl Application {
    pub fn quality_presets(&self) -> &QualityPresets {
        &self.quality
    }

    pub fn quality(&self) -> Option<&str> {
        let selected = self.quality_selected.as_deref();
        self.quality.resolve(selected).map(|p| p.name.as_str())
    }

    pub fn quality_values(&self) -> QualityValues {
        self.quality.values(self.quality_selected.as_deref())
    }

    pub fn select_quality(&mut self, name: &str) -> Result<(), String> {
        let preset = self
            .quality
            .get(name)
            .ok_or_else(|| self.quality.unknown(name))?;
        self.quality_selected = Some(preset.name.clone());
        Ok(())
    }

    pub fn quality_choice(&self) -> Option<&str> {
        self.quality_selected.as_deref()
    }

    /// Unchecked: a name the game has since removed runs the initial preset.
    pub fn restore_quality(&mut self, saved: Option<String>) {
        self.quality_selected = saved;
    }

    pub fn edit_quality_preset(&mut self, name: &str, patch: &Value) -> Result<(), String> {
        self.reread();
        let preset = self.quality.edited(name, patch)?;
        self.quality.set(preset);
        self.save()
    }

    pub fn remove_quality_preset(&mut self, name: &str) -> Result<(), String> {
        self.reread();
        if !self.quality.remove(name) {
            return Err(self.quality.unknown(name));
        }
        self.save()
    }

    pub fn default_quality(&mut self) -> Option<String> {
        self.reread();
        self.quality.default.clone()
    }

    pub fn set_default_quality(&mut self, name: Option<&str>) -> Result<(), String> {
        self.reread();
        let named = match name {
            Some(name) => Some(
                self.quality
                    .get(name)
                    .ok_or_else(|| self.quality.unknown(name))?,
            ),
            None => None,
        };
        self.quality.default = named.map(|p| p.name.clone());
        self.save()
    }
}

#[cfg(test)]
#[path = "quality_tests.rs"]
mod tests;
