//! One frame of the window, as the frame stats record it (#2205).
//!
//! A windowed run draws on its own clock: a frame runs zero or several sim ticks
//! (`shell::frame::advance_sim`), so what the window measures per frame lands beside
//! the per-tick stage timings, never inside them. The shell assembles one
//! [`WindowFrame`] per drawn frame, once, after it presented, and hands it to
//! [`record_window`]; a second consumer of the same frame (a frame log on disk)
//! takes the same value there instead of measuring again.

use super::{record_render, RenderedFrame};
use rusty_core::frame_stats::{FrameStats, RECENT};

/// What one frame of the window cost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowFrame {
    /// Wall-clock ms from the previous frame's start to this one's (the shell's
    /// `FrameClock::frame_ms`): the frame time a player feels. Outside Play the
    /// editor draws on demand, so an idle gap between two frames counts whole.
    pub wall_ms: f64,
    /// Frames the window drew over the last whole second (`FrameClock::fps`).
    pub fps: f64,
    /// What the renderer drew for the screen this frame: its counters, its CPU ms and,
    /// a frame or two late, GPU ms where the adapter has timestamps. `None` on a
    /// frame that rendered nothing (no Game or Scene view on screen yet).
    pub render: Option<RenderedFrame>,
}

/// Fold one window frame into `stats`: `wall_ms` and `fps` under keys of their own,
/// and its render as a harness frame's ([`record_render`]).
pub fn record_window(stats: &mut FrameStats, frame: &WindowFrame) {
    stats.record("wall_ms", frame.wall_ms);
    stats.record("fps", frame.fps);
    if let Some(render) = &frame.render {
        record_render(stats, render);
    }
}

/// `Debug.Stats`'s `opts.window`: the newest frames to summarise, a whole number
/// of at least 1 (more than [`RECENT`] reads the [`RECENT`] kept). `None` without one:
/// the whole session.
pub fn window_opt(opts: Option<mlua::Table>) -> mlua::Result<Option<usize>> {
    let Some(window) = opts.map(|o| o.get::<Option<f64>>("window")).transpose()? else {
        return Ok(None);
    };
    match window {
        None => Ok(None),
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(Some(n.min(RECENT as f64) as usize)),
        Some(n) => Err(mlua::Error::RuntimeError(format!(
            "Debug.Stats: window must be a whole number of frames, at least 1 (got {n})"
        ))),
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod window_tests;
