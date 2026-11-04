//! Re-reading the bound `project.rusty` (#1517, #1522, #1749): the decal budgets, the
//! audio settings and the quality presets are read from the file at each Play, before every write and
//! before every preset edit, so an edit to the file (by hand or by the agent)
//! reaches the running editor and a save never writes back a copy from before it.

use super::Application;
use crate::core::project::ProjectFile;
use crate::core::settings::audio::AudioSettings;
use crate::core::settings::decals::DecalLimits;

impl Application {
    /// Take the bound file's decal budgets and quality presets as the file stands
    /// now. A no-op unbound (the player, the harness: what opening the project
    /// read). A file that can't be read or parsed keeps the last good read, and
    /// says so once per content.
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
                        self.audio = file.audio;
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

    /// The project's decal budgets, which entering and leaving Play apply to the
    /// scene: the bound file's as it stands now ([`reread`](Self::reread)), else
    /// the ones [`set_decal_limits`](Self::set_decal_limits) gave.
    pub fn decal_limits(&mut self) -> DecalLimits {
        self.reread();
        self.decal_limits
    }

    /// The decal budgets an unbound run plays with (the player, the harness: what
    /// opening the project read).
    pub fn set_decal_limits(&mut self, limits: DecalLimits) {
        self.decal_limits = limits;
    }

    /// The project's audio settings (#1749), which entering and leaving Play apply
    /// to the audio, as last read: the Build Settings window asks every frame, so this
    /// reads no file ([`reread`](Self::reread) first for the file as it stands).
    pub fn audio_settings(&self) -> AudioSettings {
        self.audio
    }

    /// Set the project's speed of sound (the Build Settings window and
    /// `Audio.SetDefaultSpeedOfSound`), writing the
    /// bound file (if any). Not finite and positive is an error, and nothing changes.
    pub fn set_speed_of_sound(&mut self, c: f32) -> Result<(), String> {
        if !(c.is_finite() && c > 0.0) {
            return Err(format!("speed of sound must be a positive number, got {c}"));
        }
        self.reread();
        self.audio.speed_of_sound = c;
        self.save()
    }
}
