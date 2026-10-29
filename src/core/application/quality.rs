//! The project's quality presets (#1522) and the player's choice among them: the
//! presets are project config, written into the bound `project.rusty` like the
//! build settings; the choice is the player's, persisted in `Storage` by the host
//! (`core::settings::store_quality`), never in the file.

use serde_json::Value;

use super::Application;
use crate::core::settings::quality::{QualityPresets, QualityValues};

impl Application {
    /// The project's quality presets, as last read ([`reread`](Self::reread)).
    pub fn quality_presets(&self) -> &QualityPresets {
        &self.quality
    }

    /// The name of the preset that runs: the player's choice while it exists, else
    /// the presets' initial one; `None` when the game defines none.
    pub fn quality(&self) -> Option<&str> {
        let selected = self.quality_selected.as_deref();
        self.quality.resolve(selected).map(|p| p.name.as_str())
    }

    /// The values the running preset holds (the engine's own without presets).
    pub fn quality_values(&self) -> QualityValues {
        self.quality.values(self.quality_selected.as_deref())
    }

    /// Select the preset called `name` (`Graphics.SetQuality`, the editor header);
    /// a name no preset has is refused with the list.
    pub fn select_quality(&mut self, name: &str) -> Result<(), String> {
        let preset = self
            .quality
            .get(name)
            .ok_or_else(|| self.quality.unknown(name))?;
        self.quality_selected = Some(preset.name.clone());
        Ok(())
    }

    /// The player's own choice, as selected or restored: what `Storage` keeps, so a
    /// player who never chose follows the project's initial preset.
    pub fn quality_choice(&self) -> Option<&str> {
        self.quality_selected.as_deref()
    }

    /// Take the player's saved choice at boot, unchecked: a name the game has since
    /// removed runs the initial preset.
    pub fn restore_quality(&mut self, saved: Option<String>) {
        self.quality_selected = saved;
    }

    /// Write `patch` (any part of a preset's values) over the preset called `name`,
    /// or over the engine's values for a new one, as the bound file holds it now,
    /// and write the file (`Graphics.SetQualityPreset`).
    pub fn edit_quality_preset(&mut self, name: &str, patch: &Value) -> Result<(), String> {
        self.reread();
        let preset = self.quality.edited(name, patch)?;
        self.quality.set(preset);
        self.save()
    }

    /// Remove the preset called `name`, writing the bound file; a name no preset
    /// has is refused with the list. Removing the running one runs the initial one.
    pub fn remove_quality_preset(&mut self, name: &str) -> Result<(), String> {
        self.reread();
        if !self.quality.remove(name) {
            return Err(self.quality.unknown(name));
        }
        self.save()
    }

    /// The preset a player with no saved choice gets, as the file names it
    /// (`"default"`); `None` selects the middle one.
    pub fn default_quality(&mut self) -> Option<String> {
        self.reread();
        self.quality.default.clone()
    }

    /// Name the preset a player with no saved choice gets (`None`: the middle
    /// one), writing the bound file; a name no preset has is refused with the list.
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
