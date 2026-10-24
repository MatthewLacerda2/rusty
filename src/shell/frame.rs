//! src/shell/frame.rs — the frame clock, the sim advance, quit semantics, and
//! swapchain acquisition: the per-frame logic both frontends share.
//!
//! Everything but [`acquire_frame`] is window- and GPU-free, so it is unit-tested
//! directly (`frame_tests.rs`).

use std::time::{Duration, Instant};

use crate::app::{GameWorld, PlayTransition};
use crate::render::Renderer;
use wgpu::CurrentSurfaceTexture;

/// Wall-clock frame timing for the windowed loop: the raw delta handed to the sim and
/// a once-a-second FPS counter the editor shows. Platform-side only — the sim never
/// reads it (the paused/stepped mode uses the fixed dt instead).
pub struct FrameClock {
    last_frame: Instant,
    last_delta: f32,
    frame_count: u32,
    fps: f32,
    last_fps_update: Instant,
}

impl FrameClock {
    /// A clock whose first frame is measured from `now`.
    pub fn new(now: Instant) -> Self {
        Self {
            last_frame: now,
            last_delta: 0.0,
            frame_count: 0,
            fps: 60.0,
            last_fps_update: now,
        }
    }

    /// Mark a new frame at `now`; returns the seconds since the previous one.
    pub fn tick(&mut self, now: Instant) -> f32 {
        self.last_delta = now.saturating_duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.frame_count += 1;
        if now.saturating_duration_since(self.last_fps_update) >= Duration::from_secs(1) {
            self.fps = self.frame_count as f32;
            self.frame_count = 0;
            self.last_fps_update = now;
        }
        self.last_delta
    }

    /// Frames counted over the last whole second.
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// The last frame's duration in milliseconds.
    pub fn frame_ms(&self) -> f32 {
        self.last_delta * 1000.0
    }
}

/// Advance the simulation for one rendered frame, honouring the "playing-but-stepped"
/// mode (#283): not paused, a normal real-time tick with the wall-clock `delta_time`;
/// paused, [`GameWorld::paused_frame`] (only the queued fixed-dt steps move the world,
/// and a Play/Stop still takes effect). Rendering continues every frame regardless.
///
/// Every edit-mode follower (a re-written model reloading into its entities, a
/// moved bone skinning, the UI's texture sizes) hangs off [`GameWorld::edit_frame`],
/// which runs on every frame outside Play, stepped or frozen. Add a new one there,
/// not here. In Play, a tick reads the UI's texture sizes itself.
pub fn advance_sim(game: &mut GameWorld, delta_time: f32) -> PlayTransition {
    let paused = game.time().borrow().paused;
    if !paused {
        return game.tick(delta_time);
    }
    game.paused_frame()
}

/// Which frontend is hosting the game — it decides what `Application.Quit()` means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Editor,
    Player,
}

/// What the host does about a pending `Application.Quit()` this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitAction {
    None,
    /// The editor stops Play (Unity ignores `Quit` in the editor; stopping Play is
    /// the closest honest reading of "the game ended").
    StopPlay,
    /// The player closes the app; the loop's exit flushes `Storage` first.
    Exit,
}

/// The quit semantics per host. A request raised in the editor outside Play (e.g.
/// from the console REPL) is dropped: there is no game running to end.
pub fn quit_action(host: Host, requested: bool, playing: bool) -> QuitAction {
    match (requested, host) {
        (false, _) => QuitAction::None,
        (true, Host::Player) => QuitAction::Exit,
        (true, Host::Editor) if playing => QuitAction::StopPlay,
        (true, Host::Editor) => QuitAction::None,
    }
}

/// Acquire the next swapchain frame, recovering from surface loss. Returns `None`
/// when the frame should be skipped this redraw (recovered or exiting).
pub fn acquire_frame(renderer: &mut Renderer) -> Option<wgpu::SurfaceTexture> {
    let surface_frame = renderer
        .surface
        .as_ref()
        .expect("windowed renderer must have a surface")
        .get_current_texture();
    // `Lost`/`Outdated` only recover by reconfiguring the surface — which never
    // happens on the per-frame path otherwise (only on Resized), so a surface lost
    // without a resize event would log forever.
    // A suboptimal frame still presents, as it always did; the next resize or
    // `Outdated` reconfigures. Out-of-memory is no longer a surface outcome (wgpu 30
    // reports it as a device error), so nothing here exits.
    match surface_frame {
        CurrentSurfaceTexture::Success(f) | CurrentSurfaceTexture::Suboptimal(f) => Some(f),
        CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
            // Reconfigure to the swapchain's own size (never a view's rect, #355).
            renderer.reconfigure_surface();
            None
        }
        // Skip the frame and try again: the window is hidden, the GPU is slow, or an
        // error scope caught a validation error.
        CurrentSurfaceTexture::Timeout
        | CurrentSurfaceTexture::Occluded
        | CurrentSurfaceTexture::Validation => None,
    }
}

#[cfg(test)]
#[path = "frame_edit_tests.rs"]
mod frame_edit_tests;
#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
