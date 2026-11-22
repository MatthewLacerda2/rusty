//! crates/shell/src/frame/mod.rs — the frame clock, the sim advance, quit semantics, and
//! swapchain acquisition: the per-frame logic both frontends share, the command
//! channel's drain, and a `Scene.LoadAsync`'s worker and GPU warm-up while frames go
//! on.

use std::time::{Duration, Instant};

use rusty_app::{GameWorld, PlayTransition};
use rusty_render::interpolate::DrawPoses;
use rusty_render::Renderer;
use rusty_scene::SceneId;
use rusty_time::FIXED_DELTA_TIME;
use wgpu::CurrentSurfaceTexture;

pub mod load;
pub mod stats;
pub mod ticks;
pub use ticks::FixedTicks;

pub struct FrameClock {
    last_frame: Instant,
    last_delta: f32,
    frame_count: u32,
    fps: Option<f32>,
    last_fps_update: Instant,
}

impl FrameClock {
    /// A clock whose first frame is measured from `now`.
    pub fn new(now: Instant) -> Self {
        Self {
            last_frame: now,
            last_delta: 0.0,
            frame_count: 0,
            fps: None,
            last_fps_update: now,
        }
    }

    /// Mark a new frame at `now`; returns the seconds since the previous one.
    pub fn tick(&mut self, now: Instant) -> f32 {
        self.last_delta = now.saturating_duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.frame_count += 1;
        let since = now.saturating_duration_since(self.last_fps_update);
        if since >= Duration::from_secs(1) {
            self.fps = Some(self.frame_count as f32 / since.as_secs_f32());
            self.frame_count = 0;
            self.last_fps_update = now;
        }
        self.last_delta
    }

    pub fn fps(&self) -> Option<f32> {
        self.fps
    }

    /// The last frame's duration in milliseconds.
    pub fn frame_ms(&self) -> f32 {
        self.last_delta * 1000.0
    }
}

pub struct Step<'a> {
    pub ticks: &'a mut FixedTicks,
    pub poses: &'a mut DrawPoses,
}

impl<'a> Step<'a> {
    pub fn of(ticks: &'a mut FixedTicks, poses: &'a mut DrawPoses) -> Self {
        Self { ticks, poses }
    }

    pub fn clear(&mut self) {
        self.ticks.clear();
        self.poses.clear();
    }
}

/// Advance the simulation for one rendered frame of `raw` wall-clock seconds.
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
            step.clear();
            return transition;
        }
        if swapped_scene(game, scene) {
            step.clear();
            return PlayTransition::None;
        }
        step.poses.record(&game.scene().borrow());
    }
    step.poses.set_alpha(step.ticks.alpha());
    PlayTransition::None
}

pub fn swapped_scene(game: &GameWorld, before: SceneId) -> bool {
    game.scene().borrow().id() != before
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host {
    Editor,
    Player,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitAction {
    None,
    StopPlay,
    Exit,
}

/// The quit semantics per host. A request raised in the editor outside Play (e.g.
pub fn quit_action(host: Host, requested: bool, playing: bool) -> QuitAction {
    match (requested, host) {
        (false, _) => QuitAction::None,
        (true, Host::Player) => QuitAction::Exit,
        (true, Host::Editor) if playing => QuitAction::StopPlay,
        (true, Host::Editor) => QuitAction::None,
    }
}

/// Acquire the next swapchain frame, recovering from surface loss. Returns `None`
pub fn acquire_frame(renderer: &mut Renderer) -> Option<wgpu::SurfaceTexture> {
    let surface_frame = renderer
        .surface
        .as_ref()
        .expect("windowed renderer must have a surface")
        .get_current_texture();
    match surface_frame {
        CurrentSurfaceTexture::Success(f) | CurrentSurfaceTexture::Suboptimal(f) => Some(f),
        CurrentSurfaceTexture::Lost | CurrentSurfaceTexture::Outdated => {
            renderer.reconfigure_surface();
            None
        }
        CurrentSurfaceTexture::Timeout
        | CurrentSurfaceTexture::Occluded
        | CurrentSurfaceTexture::Validation => None,
    }
}

#[cfg(feature = "dev")]
pub(crate) fn drain_commands(shell: &mut crate::Shell, game: &GameWorld) {
    let Some(channel) = shell.cmd_channel.as_ref() else {
        return;
    };
    let ran = channel.drain(game.script_manager(), &game.resources.console);
    if ran > 0 && !game.scene().borrow().is_playing() {
        shell.renderer.shadow_renderer.invalidate_static_cache();
    }
}

// lint: allow(gpu-default-only) ship stub: no command channel to drain; nothing is drawn
#[cfg(not(feature = "dev"))]
pub(crate) fn drain_commands(_shell: &mut crate::Shell, _game: &GameWorld) {}

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
#[path = "load/tick_tests.rs"]
mod load_tick_tests;
#[cfg(test)]
#[path = "tick_input_tests.rs"]
mod tick_input_tests;
