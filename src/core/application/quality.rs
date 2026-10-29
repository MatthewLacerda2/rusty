//! The project's quality presets (#1522) and the player's choice among them: the
//! presets are project config, written into the bound `project.rusty` like the
//! build settings; the choice is the player's, persisted in `Storage` by the host
//! (`core::settings::store_quality`), never in the file.

use super::Application;
use crate::core::settings::quality::{QualityPreset, QualityPresets, QualityValues};

impl Application {
    /// The project's quality presets.
    pub fn quality_presets(&self) -> &QualityPresets {
        &self.quality
    }

    /// Replace the presets (an unbound run: what opening the project read), writing
    /// the bound file (if any) once.
    pub fn set_quality_presets(&mut self, presets: QualityPresets) -> Result<(), String> {
        self.quality = presets;
        self.save()
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

    /// Add `preset`, or replace the one of its name, writing the bound file.
    pub fn set_quality_preset(&mut self, preset: QualityPreset) -> Result<(), String> {
        self.quality.set(preset);
        self.save()
    }

    /// Remove the preset called `name`, writing the bound file; a name no preset
    /// has is refused with the list. Removing the running one runs the initial one.
    pub fn remove_quality_preset(&mut self, name: &str) -> Result<(), String> {
        if !self.quality.remove(name) {
            return Err(self.quality.unknown(name));
        }
        self.save()
    }
}
