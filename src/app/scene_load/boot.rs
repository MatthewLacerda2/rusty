//! src/app/scene_load/boot.rs — what a boot opens: its scene and its project.
//!
//! * **The scene** (#1618): the editor, the player and the agent sessions try a
//!   `core::project::BootPlan`'s scenes in order, so a remembered scene that no
//!   longer loads falls back to the startup scene instead of leaving the editor
//!   Untitled on every launch.
//! * **The project** (#1652): every entry point (the editor, the player, the harness
//!   and the headless session) takes `project.rusty`'s settings into its world
//!   through [`GameWorld::open_project`], once, so no boot path answers the engine's
//!   defaults where the others answer the project's.

use std::path::PathBuf;

use super::super::game::GameWorld;
use crate::core::project::{BootPlan, ProjectSettings, PROJECT_FILE};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Where a world takes its project settings from ([`GameWorld::open_project`]).
#[derive(Clone, Debug)]
pub enum ProjectSource {
    /// The project file at this path, bound: `Application`'s setters write it back
    /// (the editor and the headless session, which author the project).
    Bound(PathBuf),
    /// Settings already read, unbound: a setter changes only the running value
    /// (the player, `play` and a demo replay, which run the game and never rewrite
    /// it).
    ReadOnly(ProjectSettings),
}

impl ProjectSource {
    /// The open project's `project.rusty` (the working directory's), bound.
    pub fn project_file() -> Self {
        Self::Bound(PathBuf::from(PROJECT_FILE))
    }
}

impl GameWorld {
    /// Open the project's settings into this world: bind (or fill) `Application`
    /// from `source`, start the graphics options from the project's defaults (a
    /// player's saved choice, loaded later by the shell, still wins), and run on the
    /// project's decal budgets and quality presets. A bound file that can't be read
    /// is logged to the console and leaves the defaults. Every entry point calls it.
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
            self.resources.console.borrow_mut().error(err);
        }
        let defaults = self.resources.application.borrow().graphics_defaults();
        let graphics = self.resources.script_manager.graphics_cell();
        graphics.borrow_mut().options.rebase(defaults);
        self.apply_project_settings();
    }
}

/// Load the first of `plan`'s scenes that loads into `scene`, logging the plan's
/// warnings and every failure to `console` (`announce` also logs each attempt).
/// Returns the path loaded; `None` leaves `scene` empty, an Untitled scene. A scene
/// whose lighting sidecar alone fails is loaded, with a warning.
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
        match scene.load_from_file(path) {
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
pub(crate) mod project_tests;
