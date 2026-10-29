//! The project's decal budgets (#1517): read from the bound file at each Play, so
//! an edit to `project.rusty` reaches the next one; never written here.

use super::Application;
use crate::core::project::ProjectFile;
use crate::core::settings::decals::DecalLimits;

impl Application {
    /// The project's decal budgets, which entering and leaving Play apply to the
    /// scene. Bound, the file as it stands now, so an edit to `project.rusty` (by
    /// hand or by the agent) reaches the next Play; unbound, the ones
    /// [`set_decal_limits`] gave. A bound file that can't be read or parsed keeps
    /// the last good read, and says so once per content.
    ///
    /// [`set_decal_limits`]: Self::set_decal_limits
    pub fn decal_limits(&mut self) -> DecalLimits {
        let Some(path) = &self.path else {
            return self.decal_limits;
        };
        let why = match std::fs::read_to_string(path) {
            Ok(text) if self.decals_read_from.as_ref() == Some(&text) => return self.decal_limits,
            Ok(text) => {
                let parsed = ProjectFile::from_json(&text);
                self.decals_read_from = Some(text);
                match parsed {
                    Ok(file) => {
                        self.decal_limits = file.decals;
                        return self.decal_limits;
                    }
                    Err(err) => err,
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                format!("{} is missing", path.display())
            }
            Err(err) => format!("Failed to read {}: {err}", path.display()),
        };
        log::warn!("[Project] decals: {why}; keeping the budgets last read");
        self.decal_limits
    }

    /// The decal budgets an unbound run plays with (the player, the harness: what
    /// opening the project read).
    pub fn set_decal_limits(&mut self, limits: DecalLimits) {
        self.decal_limits = limits;
    }
}
