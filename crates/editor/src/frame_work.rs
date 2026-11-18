//! The editor's frame work that paints nothing: the one list both a drawn frame
//! ([`EditorUi::draw`]) and a frame the window cannot draw (the screen locked, the
//! window occluded or minimised) run. The shell still runs an undrawn frame (it
//! ticks the sim and answers the attach bridge's commands) but skips the egui pass,
//! so work an agent asked for through the bridge never waits on a redraw: a
//! `Lighting.GenerateAsync()` starts and runs to its end, and an edit reaches the
//! navmesh, as on a drawn frame. Background work a frame steps goes here, never in
//! a panel's draw, or a locked screen stops it.

use crate::EditorUi;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

/// Whether the window draws this frame, which sets how much of a bake's probe
/// stage it takes: one step on a drawn frame, a time budget on an undrawn one
/// (`bake::poll_undrawn`), since nothing is painted there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Paint {
    /// The window draws: [`EditorUi::draw`] calls [`EditorUi::frame_work`].
    Drawn,
    /// The swapchain refused the frame: the shell calls [`EditorUi::frame_work`]
    /// alone, never `draw`.
    Undrawn,
}

impl EditorUi {
    /// The frame's work besides painting: the edit-mode navmesh sync and, in dev
    /// builds, Generate Lighting's step (`bake::poll`, which also takes a script's
    /// request and waits on its own while the scene plays). `playing` is what
    /// `draw`'s `is_playing` is. Called once a frame, by `draw` on a drawn frame
    /// and by the shell on an undrawn one.
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
            // One diffing navmesh sync per edit-mode frame catches every edit since
            // the last one, whoever made it: a card, the gizmo, the console, MCP
            // (#992). It rebakes only changed cells; play mode has `rebake_nav`.
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
