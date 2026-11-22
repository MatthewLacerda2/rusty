use rusty_app::GameWorld;
#[cfg(feature = "dev")]
use rusty_dev::{capture::RenderedFrame, stats::WindowFrame};
#[cfg(feature = "dev")]
use rusty_render::Renderer;

use super::FrameClock;
use crate::Shell;

pub fn measured(
    shell: &mut Shell,
    game: &mut GameWorld,
    draw: impl FnOnce(&mut Shell, &mut GameWorld),
) {
    let frames_before = shell.renderer.frame_cost().frames;
    draw(shell, game);
    #[cfg(feature = "dev")]
    {
        let frame = window_frame(&mut shell.renderer, &shell.clock, frames_before);
        let in_play = game.scene().borrow().is_playing();
        let stats = game.script_manager().stats_cell();
        rusty_dev::stats::record_window(&mut stats.borrow_mut(), &frame, in_play);
        if let Some(log) = shell.frame_log.as_mut() {
            log.frame(game, &frame, shell.renderer.times_gpu());
        }
    }
    // lint: allow(gpu-default-only) marks the dev-only arguments used; nothing is drawn
    #[cfg(not(feature = "dev"))]
    let _ = frames_before;
}

/// A frame starts: the clock just read the previous frame's wall time, which goes onto that
/// frame's stats entry now, so whatever reads `Debug.Stats` during this frame sees it.
pub fn started(clock: &FrameClock, game: &GameWorld) {
    #[cfg(feature = "dev")]
    {
        let stats = game.script_manager().stats_cell();
        let last_wall_ms = f64::from(clock.frame_ms());
        rusty_dev::stats::window_ended(&mut stats.borrow_mut(), last_wall_ms);
    }
    // lint: allow(gpu-default-only) marks the dev-only arguments used; nothing is drawn
    #[cfg(not(feature = "dev"))]
    let _ = (clock, game);
}

/// The stats keep only drawn frames; the frame log still writes this one, so a bake run under
/// a locked screen can be read back.
pub fn undrawn(shell: &mut Shell, game: &GameWorld) {
    #[cfg(feature = "dev")]
    if let Some(log) = shell.frame_log.as_mut() {
        let frames = shell.renderer.frame_cost().frames;
        let frame = window_frame(&mut shell.renderer, &shell.clock, frames);
        log.frame(game, &frame, shell.renderer.times_gpu());
    }
    // lint: allow(gpu-default-only) marks the dev-only arguments used; nothing is drawn
    #[cfg(not(feature = "dev"))]
    let _ = (shell, game);
}

/// What this frame cost: the clock's reading, and the renderer's frame if it drew
/// since its count read `frames_before`.
#[cfg(feature = "dev")]
pub fn window_frame(
    renderer: &mut Renderer,
    clock: &FrameClock,
    frames_before: u64,
) -> WindowFrame {
    let cost = renderer.frame_cost();
    let render = (cost.frames != frames_before).then(|| RenderedFrame {
        counters: renderer.frame_counters,
        cpu_ms: cost.cpu_ms,
        // The window reads no stages, so none is copied on every live frame.
        stages: Vec::new(),
        gpu: renderer.take_gpu_times(),
    });
    WindowFrame {
        last_wall_ms: f64::from(clock.frame_ms()),
        fps: clock.fps().map(f64::from),
        render,
        frame: cost.frames,
    }
}

#[cfg(feature = "dev")]
pub fn start_log(host: super::Host, game: &GameWorld) -> Option<rusty_dev::frame_log::FrameLog> {
    use rusty_dev::frame_log::{FrameLog, LogHost};
    let host = match host {
        super::Host::Editor => LogHost::Editor,
        super::Host::Player => LogHost::Player,
    };
    FrameLog::for_host(host, game.resources.application.borrow().log_settings())
}

#[cfg(all(test, feature = "dev"))]
#[path = "stats_tests.rs"]
mod stats_tests;
