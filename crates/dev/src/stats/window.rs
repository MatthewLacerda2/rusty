//! One frame of the window, as the frame stats record it (#2205).
//!
//! A windowed run draws on its own clock: a frame runs zero or several sim ticks
//! (`shell::frame::advance_sim`), so what the window measures per frame lands beside
//! the per-tick stage timings, never inside them. The shell assembles one
//! [`WindowFrame`] per drawn frame, once, after the frontend drew it and before
//! the surface is presented (`shell::frame::stats::measured`), and hands it to
//! [`record_window`]; a second consumer of the same frame (a frame log on disk)
//! takes the same value there instead of measuring again.

use super::{record_render_into, RenderedFrame};
use rusty_core::frame_stats::{FrameStats, RECENT};

/// What one frame of the window cost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowFrame {
    /// The previous window frame's wall time: ms from its start to this one's (the
    /// shell's `FrameClock::frame_ms`, read as this frame starts), the frame time a
    /// player feels. It is the previous frame's, not this one's: the shell records
    /// it on that frame as this one starts ([`window_ended`]), as the frame log
    /// does. Outside Play the editor draws on demand, so an idle gap between two
    /// frames counts whole.
    pub last_wall_ms: f64,
    /// The clock's last whole-second reading (`FrameClock::fps`): the frames drawn
    /// since its previous reading, over the seconds between them. `None` until the
    /// window's first second is up, so no made-up rate is recorded.
    pub fps: Option<f64>,
    /// What the renderer drew for the screen this frame: its counters, its CPU ms and,
    /// a frame or two late, GPU ms where the adapter has timestamps. `None` on a
    /// frame that rendered nothing (no Game or Scene view on screen yet).
    pub render: Option<RenderedFrame>,
    /// The renderer's drawn-frame count after this frame (`FrameCost::frames`):
    /// this frame's index when it drew, the last drawn one's otherwise. The GPU
    /// times in `render` carry the index of the frame they measure
    /// (`GpuTimes::frame`), normally a frame or two before this one.
    pub frame: u64,
}

/// Fold one window frame into `stats`: its `fps` and its render, as a harness
/// frame's ([`record_render_into`]). Its `wall_ms` is not known yet: the shell
/// records it on this frame when the next one starts ([`window_ended`]), so the
/// newest frame has none. A frame drawn in Play (`in_play`) joins the session's
/// series; any other reaches only the recent ring, so `Debug.Stats()` keeps the
/// last Play session after Stop while a window and the Stats overlay read the
/// editor's frames.
pub fn record_window(stats: &mut FrameStats, frame: &WindowFrame, in_play: bool) {
    stats.record_frame(in_play, |drawn| {
        if let Some(fps) = frame.fps {
            drawn.record("fps", fps);
        }
        if let Some(render) = &frame.render {
            record_render_into(drawn, render);
        }
    });
}

/// A window frame starts, and `last_wall_ms` (the frame clock's reading) is the
/// previous one's wall time: it goes onto the newest recorded frame as `wall_ms`,
/// before this frame's work, so anything reading the stats during the frame (the
/// REPL, a script's `Update`, the Stats overlay) sees it. A frame the window could
/// not show records no entry, so the time of such a frame, ended when the next
/// starts, finds that entry already ended and is dropped.
pub fn window_ended(stats: &mut FrameStats, last_wall_ms: f64) {
    stats.frame_ended("wall_ms", last_wall_ms);
}

/// The option keys `Debug.Stats` takes.
const OPTIONS: &[&str] = &["window"];

/// `Debug.Stats`'s `opts.window`: the newest frames to summarise, a whole number
/// of at least 1 (more than [`RECENT`], `math.huge` included, reads the [`RECENT`]
/// kept). `None` without one: the whole session. Any other key raises, naming the
/// one option.
pub fn window_opt(opts: Option<mlua::Table>) -> mlua::Result<Option<usize>> {
    let Some(opts) = opts else {
        return Ok(None);
    };
    known_keys(&opts, "Stats", OPTIONS)?;
    let window = whole_count(&opts, "Stats", "window")?;
    Ok(window.map(|n| n.min(RECENT)))
}

/// Raise unless every key of `opts` is one of `known`, naming them: the check
/// each `Debug.<call>{ … }` with named options makes.
pub fn known_keys(opts: &mlua::Table, call: &str, known: &[&str]) -> mlua::Result<()> {
    for pair in opts.pairs::<mlua::Value, mlua::Value>() {
        let (key, _) = pair?;
        let name = key.to_string()?;
        if !known.contains(&name.as_str()) {
            return Err(mlua::Error::RuntimeError(format!(
                "Debug.{call}: unknown option {name:?}; expected one of {}",
                known.join(", ")
            )));
        }
    }
    Ok(())
}

/// `opts[key]` as a count: a whole number of at least 1, or `math.huge` for
/// "all" (`usize::MAX`); `None` when absent. Anything else raises.
pub fn whole_count(opts: &mlua::Table, call: &str, key: &str) -> mlua::Result<Option<usize>> {
    match opts.get::<Option<f64>>(key)? {
        None => Ok(None),
        Some(n) if n == f64::INFINITY => Ok(Some(usize::MAX)),
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(Some(n as usize)),
        Some(n) => Err(mlua::Error::RuntimeError(format!(
            "Debug.{call}: {key} must be a whole number, at least 1, or math.huge (got {n})"
        ))),
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod window_tests;
