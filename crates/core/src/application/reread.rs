//! The bound `project.rusty`'s budgets, presets, audio, log and render-scale settings are
//! reread at each Play and before each write, so a hand edit reaches the running editor
//! and a save never writes back a copy from before it.

use super::Application;
use crate::project::ProjectFile;
use crate::settings::audio::AudioSettings;
use crate::settings::decals::DecalLimits;
use crate::settings::origin::OriginSettings;

impl Application {
    /// An unreadable file keeps the last good read and warns once per content.
    pub fn reread(&mut self) {
        let Some(path) = &self.path else {
            return;
        };
        let why = match std::fs::read_to_string(path) {
            Ok(text) if self.read_from.as_ref() == Some(&text) => return,
            Ok(text) => {
                let parsed = ProjectFile::from_json(&text);
                self.read_from = Some(text);
                match parsed {
                    Ok(file) => {
                        self.decal_limits = file.decals;
                        self.quality = file.quality;
                        (self.audio, self.logs) = (file.audio, file.logs);
                        self.graphics = file.graphics;
                        self.floating_origin = file.floating_origin;
                        return;
                    }
                    Err(err) => err,
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                format!("{} is missing", path.display())
            }
            Err(err) => format!("Failed to read {}: {err}", path.display()),
        };
        log::warn!("[Project] {why}; keeping the decal budgets and quality presets last read");
    }

    /// Reread with `decal_limits`, which Play's settings pass calls first.
    pub fn origin_settings(&self) -> OriginSettings {
        self.floating_origin
    }

    pub fn decal_limits(&mut self) -> DecalLimits {
        self.reread();
        self.decal_limits
    }

    pub fn set_decal_limits(&mut self, limits: DecalLimits) {
        self.decal_limits = limits;
    }

    /// Reads no file, unlike `decal_limits`: the Build Settings window asks every frame.
    pub fn audio_settings(&self) -> AudioSettings {
        self.audio
    }

    pub fn set_speed_of_sound(&mut self, c: f32) -> Result<(), String> {
        if !(c.is_finite() && c > 0.0) {
            return Err(format!("speed of sound must be a positive number, got {c}"));
        }
        self.reread();
        self.audio.speed_of_sound = c;
        self.save()
    }
}
