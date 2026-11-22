//! One window frame into the frame stats: a frame runs zero or several sim ticks, so what
//! it costs lands beside the per-tick timings, never inside them.

use super::{record_render_into, RenderedFrame};
use rusty_core::frame_stats::{FrameStats, RECENT};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowFrame {
    /// The previous frame's wall ms, start to start, not this one's: [`window_ended`]
    /// records it on that frame.
    pub last_wall_ms: f64,
    pub fps: Option<f64>,
    pub render: Option<RenderedFrame>,
    pub frame: u64,
}

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

pub fn window_ended(stats: &mut FrameStats, last_wall_ms: f64) {
    stats.frame_ended("wall_ms", last_wall_ms);
}

const OPTIONS: &[&str] = &["window"];

pub fn window_opt(opts: Option<mlua::Table>) -> mlua::Result<Option<usize>> {
    let Some(opts) = opts else {
        return Ok(None);
    };
    known_keys(&opts, "Stats", OPTIONS)?;
    let window = whole_count(&opts, "Stats", "window")?;
    Ok(window.map(|n| n.min(RECENT)))
}

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
