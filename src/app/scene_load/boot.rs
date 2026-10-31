//! src/app/scene_load/boot.rs — load the scene a boot opens on (#1618): the editor,
//! the player and the agent sessions try a `core::project::BootPlan`'s scenes in
//! order, so a remembered scene that no longer loads falls back to the startup scene
//! instead of leaving the editor Untitled on every launch.

use crate::core::project::BootPlan;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

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
