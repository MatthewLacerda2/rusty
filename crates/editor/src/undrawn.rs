//! The editor's frame work that paints nothing, on a frame the window cannot draw:
//! the screen locked, the window occluded or minimised. The shell still runs that
//! frame (it ticks the sim and answers the attach bridge's commands) but skips the
//! paint, and with it [`EditorUi::draw`], which is where this work otherwise runs.
//! So work an agent asked for through the bridge never waits on a redraw: a
//! `Lighting.GenerateAsync()` starts and runs to its end, and an edit reaches the
//! navmesh, as on a drawn frame.

use crate::EditorUi;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

impl EditorUi {
    /// What [`EditorUi::draw`] does on a frame besides painting, for a frame the
    /// window did not draw: the edit-mode navmesh sync and, in dev builds, Generate
    /// Lighting's step (`bake::poll`, which also takes a script's request). Never
    /// call both on one frame. `playing` is what `draw`'s `is_playing` would be.
    pub fn undrawn_frame(
        &mut self,
        ctx: &egui::Context,
        scene: &mut Scene,
        console: &mut ConsoleLogs,
        nav: &mut NavigationGraph,
        playing: bool,
    ) {
        if !playing {
            nav.sync(scene);
        }
        #[cfg(feature = "dev")]
        crate::inspector::assets::scene::bake::poll(self, ctx, scene, console, nav);
        #[cfg(not(feature = "dev"))]
        let _ = (ctx, console);
    }
}

#[cfg(test)]
#[path = "undrawn_tests.rs"]
mod tests;
