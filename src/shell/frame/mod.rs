//! src/shell/frame/mod.rs — the frame clock, the sim advance, quit semantics, and
//! swapchain acquisition: the per-frame logic both frontends share.
//!
//! Everything but [`acquire_frame`] is window- and GPU-free, so it is unit-tested
//! directly (`frame_tests.rs`).

use std::time::{Duration, Instant};

use crate::app::{GameWorld, PlayTransition};
use crate::render::interpolate::DrawPoses;
use crate::render::Renderer;
use crate::scene::SceneId;
use crate::time::FIXED_DELTA_TIME;
use wgpu::CurrentSurfaceTexture;

pub mod ticks;
pub use ticks::FixedTicks;

/// Wall-clock frame timing for the windowed loop: the raw delta handed to
/// [`advance_sim`] and a once-a-second FPS counter the editor shows. Platform-side
/// only — the sim never reads it (in Play it ticks on the fixed dt).
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

/// What the shell keeps across frames for [`advance_sim`]: the time owed to the
/// fixed tick and the poses drawing blends between.
pub struct Step<'a> {
    pub ticks: &'a mut FixedTicks,
    pub poses: &'a mut DrawPoses,
}

impl<'a> Step<'a> {
    pub fn of(ticks: &'a mut FixedTicks, poses: &'a mut DrawPoses) -> Self {
        Self { ticks, poses }
    }

    /// The sim is off the tick this frame (or came back from a scene load): nothing
    /// owed, everything drawn as it is.
    pub fn clear(&mut self) {
        self.ticks.clear();
        self.poses.clear();
    }
}

/// Advance the simulation for one rendered frame of `raw` wall-clock seconds (#1877).
///
/// * **Play:** on the fixed tick. `ticks` takes the frame's time and runs every whole
///   [`FIXED_DELTA_TIME`] tick now owed (none on a frame shorter than a tick, at most
///   20 after a hitch); `poses` records each one and is told how far the frame is into
///   the next, so drawing blends between the last two ticks. A frame that owes no tick
///   leaves the input it saw pending for the next one. A tick that swaps the scene
///   (a play-mode `Scene.Load`, [`swapped_scene`]) is the frame's last, and the clock
///   starts afresh after it.
/// * **Entering or leaving Play:** one fixed tick runs the transition now (`Start` and
///   the first `Update`, or the Stop), and the clock starts afresh.
/// * **Edit mode:** one frame of `raw` time, as the editor always ran it.
/// * **Paused** ("playing-but-stepped", #283): [`GameWorld::paused_frame`] (only the
///   queued fixed-dt steps move the world, and a Play/Stop still takes effect).
///
/// Every edit-mode follower (a re-written model reloading into its entities, a
/// moved bone skinning, the UI's texture sizes) hangs off [`GameWorld::edit_frame`],
/// which runs on every frame outside Play, stepped or frozen. Add a new one there,
/// not here. In Play, a tick reads the UI's texture sizes itself.
pub fn advance_sim(game: &mut GameWorld, mut step: Step<'_>, raw: f32) -> PlayTransition {
    if game.time().borrow().paused {
        step.clear();
        return game.paused_frame();
    }
    let playing = game.scene().borrow().is_playing();
    let heading = game.resources.application.borrow().play_heading();
    if !playing && !heading {
        step.clear();
        return game.tick(raw);
    }
    if playing != heading {
        step.clear();
        return game.tick(FIXED_DELTA_TIME);
    }
    for _ in 0..step.ticks.owe(raw) {
        let scene = game.scene().borrow().id();
        let transition = game.tick(FIXED_DELTA_TIME);
        if transition != PlayTransition::None {
            // A Stop a tick ran (a script's `Editor.ExitPlay`): the rest is edit mode.
            step.clear();
            return transition;
        }
        if swapped_scene(game, scene) {
            // The load's time is the frame the sim came back on, as a Play entry's.
            step.clear();
            return PlayTransition::None;
        }
        step.poses.record(&game.scene().borrow());
    }
    step.poses.set_alpha(step.ticks.alpha());
    PlayTransition::None
}

/// Whether the tick that just ran swapped the scene `before` names (a play-mode
/// `Scene.Load` landed at its tail): the World's runtime identity changed
/// (`Scene::swap_in` renews it). Such a tick ends its frame's ticks and restarts the
/// clock ([`Step::clear`]): the ticks still owed would run the new scene before any
/// frame drew it, the next frame's time holds the load itself, and no pose of the
/// outgoing scene is blended into the new one's first frames.
pub fn swapped_scene(game: &GameWorld, before: SceneId) -> bool {
    game.scene().borrow().id() != before
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

/// One frame through [`advance_sim`] with a fresh accumulator: each frame's ticks
/// are its own whole ticks (a test of a single frame needs nothing kept).
#[cfg(test)]
fn advance(game: &mut GameWorld, raw: f32) -> PlayTransition {
    let (mut ticks, mut poses) = Default::default();
    advance_sim(game, Step::of(&mut ticks, &mut poses), raw)
}

#[cfg(test)]
#[path = "fixed_tick_tests.rs"]
mod fixed_tick_tests;
#[cfg(test)]
#[path = "frame_edit_tests.rs"]
mod frame_edit_tests;
#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
#[cfg(test)]
#[path = "load_tick_tests.rs"]
mod load_tick_tests;
#[cfg(test)]
#[path = "tick_input_tests.rs"]
mod tick_input_tests;
