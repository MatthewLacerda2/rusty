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
    pub fn new(now: Instant) -> Self {
        Self {
            last_frame: now,
            last_delta: 0.0,
            frame_count: 0,
            fps: None,
            last_fps_update: now,
        }
    }

    /// The seconds since the previous frame, marking one at `now`.
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

    /// In milliseconds.
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

/// An edit-mode follower goes in `GameWorld::edit_frame`, which
/// runs every frame outside Play, stepped or frozen, not here.
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
            // A tick left Play: the rest is edit mode.
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

/// A tick that swaps the scene ends its frame's ticks: the rest would run the new scene
/// before a frame drew it, and blend the old scene's poses into its first frames.
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

pub fn quit_action(host: Host, requested: bool, playing: bool) -> QuitAction {
    match (requested, host) {
        (false, _) => QuitAction::None,
        (true, Host::Player) => QuitAction::Exit,
        (true, Host::Editor) if playing => QuitAction::StopPlay,
        (true, Host::Editor) => QuitAction::None,
    }
}

pub fn acquire_frame(renderer: &mut Renderer) -> Option<wgpu::SurfaceTexture> {
    let surface_frame = renderer
        .surface
        .as_ref()
        .expect("windowed renderer must have a surface")
        .get_current_texture();
    match surface_frame {
        CurrentSurfaceTexture::Success(f) | CurrentSurfaceTexture::Suboptimal(f) => Some(f),
        // Only a reconfigure recovers these: without one here, a surface lost unresized stays lost.
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
    // A command outside Play may have moved static geometry: drop the static shadow bake.
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
#[path = "teleport_tests.rs"]
mod teleport_tests;
#[cfg(test)]
#[path = "tick_input_tests.rs"]
mod tick_input_tests;
