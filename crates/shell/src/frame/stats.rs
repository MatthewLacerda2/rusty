//! Recording the window's frames into the frame stats (#2205), dev builds only: a
//! build without the `dev` layer records nothing, and these calls do nothing there.
//!
//! `Debug.Stats()` reads what the dev layer's schedule probe records per sim tick;
//! this adds what only the window knows, per drawn frame: its wall-clock frame time
//! and rate, and what the renderer drew for the screen (`rusty_dev::stats::WindowFrame`).
//! [`measured`] wraps each frame's draw, once per frame the loop draws, and hands
//! the same frame to the rolling frame log on disk when one runs (#2206), which
//! [`undrawn`] also feeds on a frame the window could not show.

use rusty_app::GameWorld;
#[cfg(feature = "dev")]
use rusty_dev::{capture::RenderedFrame, stats::WindowFrame};
#[cfg(feature = "dev")]
use {super::FrameClock, rusty_render::Renderer};

use crate::Shell;

/// Run the frontend's `draw` of this frame, then assemble its `WindowFrame` and
/// fold it into the stats `Debug.Stats()` reads: a Play frame into the session, any
/// frame into the recent ring a window reads. A frame the renderer drew carries
/// its counters, its CPU ms and the GPU times resolved since the last one; GPU
/// times wait for a frame that drew. The frame log on disk (#2206) then writes the
/// same `WindowFrame`, after the stats, so their newest frame is this one.
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
    #[cfg(not(feature = "dev"))]
    let _ = frames_before;
}

/// A frame the window could not show (a locked screen): the stats keep only drawn
/// frames, so it only ends the frame before it (its wall time goes there), but the frame log on disk (#2206) writes it, with its console lines,
/// so a bake run while the screen is locked can still be looked back at.
pub fn undrawn(shell: &mut Shell, game: &GameWorld) {
    #[cfg(feature = "dev")]
    {
        let frames = shell.renderer.frame_cost().frames;
        let frame = window_frame(&mut shell.renderer, &shell.clock, frames);
        let stats = game.script_manager().stats_cell();
        rusty_dev::stats::window_ended(&mut stats.borrow_mut(), &frame);
        if let Some(log) = shell.frame_log.as_mut() {
            log.frame(game, &frame, shell.renderer.times_gpu());
        }
    }
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
        gpu: renderer.take_gpu_times(),
    });
    WindowFrame {
        last_wall_ms: f64::from(clock.frame_ms()),
        fps: clock.fps().map(f64::from),
        render,
        frame: cost.frames,
    }
}

/// Start the frame log (#2206) when the project has `host` write one: the
/// editor by default, a `player` dev build only when its settings say so.
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
