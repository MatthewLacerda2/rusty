//! One frame's records as JSON lines (#2206): where the frame was, then what it
//! cost, what it drew, and what was logged beside it.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use rusty_app::GameWorld;
use rusty_core::frame_stats::{FrameStats, Series};
use rusty_core::settings::logs::LogChannels;
use rusty_scene::game_camera_from_scene;
use rusty_script::scripting::LogLevel;

use super::SYSTEM_MS;
use crate::stats::WindowFrame;

/// What places a record: the wall-clock time, the tick, Play or not, the active
/// scene, and the game camera's position and facing.
pub(super) struct Place {
    t: u64,
    tick: u64,
    rest: Map<String, Value>,
}

impl Place {
    pub(super) fn of(game: &GameWorld, t: u64) -> Self {
        let scene = game.scene().borrow();
        let cam = game_camera_from_scene(&game.camera().borrow(), &scene);
        let path = game.script_manager().scene_path_cell().borrow().clone();
        let mut rest = Map::new();
        rest.insert("play".into(), json!(scene.is_playing()));
        rest.insert("scene".into(), json!(path.unwrap_or_default()));
        rest.insert("cam".into(), round3(json!(cam.position.to_array())));
        rest.insert("fwd".into(), round3(json!(cam.forward().to_array())));
        let tick = game.play_frame();
        Self { t, tick, rest }
    }

    /// A record of channel `ch`: the head ([`line`]), then the place and `body`.
    fn record(&self, ch: &str, head: &[(&str, Value)], body: Map<String, Value>) -> String {
        let mut rest = self.rest.clone();
        rest.extend(body);
        line(ch, self.t, self.tick, head, rest)
    }

    /// The wall-clock time and tick alone: a line that is no frame.
    fn short(&self, ch: &str, body: Map<String, Value>) -> String {
        line(ch, self.t, self.tick, &[], body)
    }
}

/// One line: its head, `{"ch":…,"t":…,"tick":…` then `head`'s scalars in order,
/// written first so a reader can filter on them without parsing the line
/// (`read::Head`), then `body`'s fields.
pub(super) fn line(
    ch: &str,
    t: u64,
    tick: u64,
    head: &[(&str, Value)],
    body: Map<String, Value>,
) -> String {
    use std::fmt::Write;
    let mut out = format!(r#"{{"ch":{},"t":{t},"tick":{tick}"#, json!(ch));
    for (key, value) in head {
        let _ = write!(out, r#","{key}":{value}"#);
    }
    let body = Value::Object(body).to_string();
    if body.len() > 2 {
        out.push(',');
        out.push_str(&body[1..]);
    } else {
        out.push('}');
    }
    out
}

/// The frame's record: its index (`frame`), its clock, the ticks it ran and their
/// CPU ms (`sim_ms`), what the renderer drew and cost, and, per channel, its
/// systems over [`SYSTEM_MS`]. The GPU times that resolved this frame ride on it
/// under `gpu_frame`, the index of the earlier frame they measure: the reader
/// moves them onto that frame's record (`read`).
pub(super) fn frame(
    place: &Place,
    frame: &WindowFrame,
    last: &FrameStats,
    channels: LogChannels,
) -> String {
    let mut head = vec![
        ("frame", json!(frame.frame)),
        ("ms", round3(json!(frame.wall_ms))),
    ];
    let mut body = Map::new();
    if let Some(fps) = frame.fps {
        body.insert("fps".into(), round3(json!(fps)));
    }
    body.insert("ticks".into(), json!(last.frames));
    if let Some(sim) = last.get("frame_ms") {
        body.insert("sim_ms".into(), round3(json!(total(sim))));
    }
    if let Some(render) = &frame.render {
        let c = &render.counters;
        if let Some(gpu) = &render.gpu {
            head.push(("gpu_frame", json!(gpu.frame)));
        }
        head.push(("cpu_ms", round3(json!(render.cpu_ms))));
        if let Some(gpu) = &render.gpu {
            body.insert("gpu_ms".into(), round3(json!(gpu.total_ms())));
            if channels.gpu_passes {
                let mut passes = BTreeMap::<&str, f64>::new();
                for (pass, ms) in &gpu.passes {
                    *passes.entry(pass.name()).or_default() += ms;
                }
                body.insert("gpu_passes".into(), rounded(passes));
            }
        }
        body.insert("draws".into(), json!(c.draw_calls));
        body.insert("tris".into(), json!(c.triangles));
        body.insert("visible".into(), json!(c.visible_entities));
        body.insert("culled".into(), json!(c.culled_entities));
    }
    if channels.systems {
        let heavy: BTreeMap<&str, f64> = last
            .systems
            .iter()
            .map(|(name, s)| (name.as_str(), total(s)))
            .filter(|(_, ms)| *ms >= SYSTEM_MS)
            .collect();
        if !heavy.is_empty() {
            body.insert("systems".into(), rounded(heavy));
        }
    }
    place.record("frames", &head, body)
}

/// A load that finished this frame: its total and each step's ms.
pub(super) fn load(place: &Place, last: &FrameStats) -> Option<String> {
    let total_ms = last.get("load_ms")?;
    let steps: BTreeMap<&str, f64> = last
        .metrics
        .iter()
        .filter_map(|(key, s)| {
            let step = key.strip_prefix("load_")?.strip_suffix("_ms")?;
            Some((step, s.last)).filter(|_| !step.is_empty())
        })
        .collect();
    let mut body = Map::new();
    body.insert("load_ms".into(), round3(json!(total_ms.last)));
    body.insert("steps".into(), rounded(steps));
    Some(place.record("loads", &[], body))
}

/// A console line, with its level.
pub(super) fn console(place: &Place, msg: &str, level: &LogLevel) -> String {
    let level = match level {
        LogLevel::Info => "info",
        LogLevel::Warning => "warn",
        LogLevel::Error => "error",
    };
    let mut body = Map::new();
    body.insert("level".into(), json!(level));
    body.insert("msg".into(), json!(msg));
    place.short("console", body)
}

/// An audio event, as `Debug.AudioEvents` returns it.
pub(super) fn audio(place: &Place, event: &impl serde::Serialize) -> Option<String> {
    match serde_json::to_value(event).ok()? {
        Value::Object(body) => Some(place.short("audio", body)),
        _ => None,
    }
}

/// A tick metric's total over the frame's ticks.
fn total(s: &Series) -> f64 {
    s.avg * s.samples as f64
}

/// `value` to the microsecond (three decimals): the log's bytes go on history,
/// not on digits nobody reads.
fn round3(value: Value) -> Value {
    match value {
        Value::Number(n) => n
            .as_f64()
            .map_or(Value::Null, |v| json!((v * 1000.0).round() / 1000.0)),
        Value::Array(items) => Value::Array(items.into_iter().map(round3).collect()),
        other => other,
    }
}

fn rounded(map: BTreeMap<&str, f64>) -> Value {
    Value::Object(
        map.into_iter()
            .map(|(k, v)| (k.to_string(), round3(json!(v))))
            .collect(),
    )
}
