//! The editor's frame work that paints nothing, run by a drawn frame
//! ([`EditorUi::draw`]) and by one the window cannot draw (the screen locked, the
//! window occluded or minimised), where the shell skips the egui pass. Background
//! work a frame steps goes here, never in a panel's draw, or a locked screen stops it.

use crate::EditorUi;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    /// The window draws: [`EditorUi::draw`] calls [`EditorUi::frame_work`].
    Drawn,
    /// The swapchain refused the frame: the shell calls [`EditorUi::frame_work`]
    /// alone, never `draw`.
    Undrawn,
}

impl EditorUi {
    #[expect(
        clippy::too_many_arguments,
        reason = "distinct mutable borrows, as `draw`"
    )]
    pub fn frame_work(
        &mut self,
        ctx: &egui::Context,
        scene: &mut Scene,
        console: &mut ConsoleLogs,
        nav: &mut NavigationGraph,
        playing: bool,
        paint: Paint,
    ) {
        // A diff since the last sync catches every edit, whoever made it, rebaking
        // only changed cells; play mode has `rebake_nav`.
        if !playing {
            nav.sync(scene);
        }
        #[cfg(feature = "dev")]
        {
            use crate::inspector::assets::scene::{bake, bake_queue};
            bake_queue::step(self, scene, console, nav);
            match paint {
                Paint::Drawn => bake::poll(self, ctx, scene, console, nav),
                Paint::Undrawn => bake::poll_undrawn(self, ctx, scene, console, nav),
            }
            bake_queue::wake(self, ctx, scene);
        }
        // lint: allow(gpu-default-only) marks the dev-only arguments used; nothing is drawn
        #[cfg(not(feature = "dev"))]
        let _ = (ctx, console, paint);
    }
}

#[cfg(test)]
#[path = "frame_work_tests.rs"]
mod tests;
