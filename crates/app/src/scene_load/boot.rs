use std::path::PathBuf;

use super::super::game::GameWorld;
use rusty_core::project::{BootPlan, Opened, ProjectSettings, PROJECT_FILE};
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

#[derive(Clone, Debug)]
pub enum ProjectSource {
    /// `Application`'s setters write the file back: hosts that author the project.
    Bound(PathBuf),
    /// Setters change only the running value: hosts that run the game.
    ReadOnly(ProjectSettings),
}

impl ProjectSource {
    /// `opened`'s `project.rusty`, bound by absolute path.
    pub fn bound(opened: &Opened) -> Self {
        Self::Bound(opened.root.join(PROJECT_FILE))
    }

    pub fn defaults() -> Self {
        Self::ReadOnly(ProjectSettings::default())
    }

    pub fn read_only(opened: &Opened) -> Self {
        Self::ReadOnly(opened.settings.clone())
    }
}

impl GameWorld {
    /// A player's saved graphics choice, loaded later by the shell, still wins.
    pub fn open_project(&self, source: ProjectSource) {
        let opened = {
            let mut app = self.resources.application.borrow_mut();
            match source {
                ProjectSource::Bound(path) => app.open(path),
                ProjectSource::ReadOnly(settings) => {
                    app.run_with(settings);
                    Ok(())
                }
            }
        };
        if let Err(err) = opened {
            log::error!("[Project] {err}; running on the default settings");
        }
        let defaults = self.resources.application.borrow().graphics_defaults();
        let graphics = self.resources.script_manager.graphics_cell();
        graphics.borrow_mut().options.rebase(defaults);
        self.apply_project_settings();
    }
}

impl GameWorld {
    /// Rereads a bound file, on entering Play and on Stop, so a hand edit reaches the next
    /// Play.
    pub fn apply_project_settings(&self) {
        let mut app = self.resources.application.borrow_mut();
        let limits = app.decal_limits();
        self.world.scene.borrow_mut().decals.set_limits(limits);
        let c = app.audio_settings().speed_of_sound; // reread by `decal_limits`
        self.resources.audio.borrow_mut().set_speed_of_sound(c);
        let graphics = self.resources.script_manager.graphics_cell();
        let mut live = graphics.borrow_mut();
        live.quality = app.quality_values();
        live.render_scale.default = app.render_scale_default();
        self.resources.nav.borrow().reset_path_queue();
    }
}

/// Load the first of `plan`'s scenes that loads, logging the plan's warnings and every
/// failure (`announce` also logs each attempt). `None` leaves `scene` empty (Untitled);
/// a scene whose lighting sidecar alone fails still loads, with a warning.
pub fn load_boot_scene(
    scene: &mut Scene,
    console: &mut ConsoleLogs,
    plan: &BootPlan,
    announce: bool,
) -> Option<String> {
    for warning in &plan.warnings {
        console.warn(format!("[Scene] {warning}"));
    }
    for path in &plan.scenes {
        if announce {
            console.info(format!("Loading scene from {path}..."));
        }
        match rusty_edit::load_from_file(scene, path) {
            Ok(()) => return Some(path.clone()),
            Err(err) if err.replaced_world() => {
                console.warn(format!("Scene {path}: {err}"));
                return Some(path.clone());
            }
            Err(err) => console.error(format!("Failed to load scene {path}: {err}")),
        }
    }
    None
}

#[cfg(test)]
#[path = "boot_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "project_tests.rs"]
mod project_tests;
