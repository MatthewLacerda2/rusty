//! The editor's frame work that paints nothing: the one list both a drawn frame
//! ([`EditorUi::draw`]) and a frame the window cannot draw (the screen locked, the
//! window occluded or minimised) run. The shell still runs an undrawn frame (it
//! ticks the sim and answers the attach bridge's commands) but skips the egui pass,

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
        if !playing {
            nav.sync(scene);
        }
        #[cfg(feature = "dev")]
        {
            use crate::inspector::assets::scene::bake;
            match paint {
                Paint::Drawn => bake::poll(self, ctx, scene, console, nav),
                Paint::Undrawn => bake::poll_undrawn(self, ctx, scene, console, nav),
            }
        }
        // lint: allow(gpu-default-only) marks the dev-only arguments used; nothing is drawn
        #[cfg(not(feature = "dev"))]
        let _ = (ctx, console, paint);
    }
}

#[cfg(test)]
#[path = "frame_work_tests.rs"]
mod tests;
