//! crates/core/src/frame_stats.rs — per-frame perf counters and timings for agents (#433).
//!
//! An agent never *feels* the frame rate, so it needs numbers: CPU milliseconds per
//! schedule stage and per system, render counters (draw calls, triangles, culled
//! entities, dropped lights) and world counters (entities, bodies, agents, scripts).
//! [`FrameStats`] is where those numbers land — a flat, name-keyed set of running
//! [`Series`] (last / min / avg / max over the frames recorded) that `Debug.Stats()`,
//! the harness's `results.json` and scenario budgets all read.
//!
//! **Now, not the session.** Beside each series the last [`RECENT`] values are kept,
//! so [`FrameStats::window`] can summarise only the newest frames: what
//! `Debug.Stats{ window = n }` and the Game view's Stats overlay read. A window
//! counts each metric's own samples: a render counter's are drawn frames, a stage
//! timing's are sim ticks (a windowed frame runs zero or several).
//!
//! This is plain data. **Nothing here reads a clock**: the timings are measured by the
//! dev layer (`dev::stats`) around each system and written in, and the sim never reads
//! them back, so the determinism rule is untouched. Keys ending in `_ms` are
//! wall-clock timings and differ run to run; every other key is a count and is
//! deterministic for a deterministic run — [`FrameStats::to_json`] can split the two.

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;
use serde_json::{json, Value};

/// A running summary of one metric over the frames it was recorded on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Series {
    pub last: f64,
    pub min: f64,
    pub avg: f64,
    pub max: f64,
    /// How many frames recorded this metric.
    pub samples: u64,
}

impl Series {
    /// Fold one frame's value in.
    pub fn record(&mut self, value: f64) {
        if self.samples == 0 {
            (self.min, self.max) = (value, value);
        } else {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        }
        self.samples += 1;
        self.avg += (value - self.avg) / self.samples as f64;
        self.last = value;
    }
}

/// How many of each metric's newest values are kept for [`FrameStats::window`]:
/// ten seconds of a 60 Hz tick or window. A wider window is clamped to it.
pub const RECENT: usize = 600;

/// Every metric recorded since the stats were last reset (a Play session).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Sim frames (ticks) recorded.
    pub frames: u64,
    /// Frames drawn and recorded with their render counters: windowed frames, or
    /// headless frames something rendered. Not `frames`: a windowed frame runs zero
    /// or several ticks, and a headless tick need not draw.
    pub drawn_frames: u64,
    /// Stage timings, render counters and world counters, by name.
    pub metrics: BTreeMap<String, Series>,
    /// Per-system CPU milliseconds, by system name — diagnostic, not budgetable.
    pub systems: BTreeMap<String, Series>,
    /// GPU milliseconds per render-pass kind, by its stable name (#835). Empty where
    /// the adapter has no timestamps — absent, never zero.
    pub gpu_passes: BTreeMap<String, Series>,
    /// Each series' newest values, oldest first, at most [`RECENT`].
    recent: Recent,
}

/// The newest values of every series in [`FrameStats`]' three maps.
#[derive(Clone, Debug, Default, PartialEq)]
struct Recent {
    metrics: BTreeMap<String, VecDeque<f64>>,
    systems: BTreeMap<String, VecDeque<f64>>,
    gpu_passes: BTreeMap<String, VecDeque<f64>>,
}

/// Whether `key` names a wall-clock timing (non-deterministic) rather than a count:
/// milliseconds or microseconds (`render_extract_us`, `shadow_gather_us`), and the
/// window's frame rate (`fps`).
fn is_timing(key: &str) -> bool {
    key.ends_with("_ms") || key.ends_with("_us") || key == "fps"
}

impl FrameStats {
    /// Record `value` for metric `key` this frame.
    pub fn record(&mut self, key: &str, value: f64) {
        record_into(&mut self.metrics, &mut self.recent.metrics, key, value);
    }

    /// Record `ms` of CPU time for the system named `name` this frame.
    pub fn record_system(&mut self, name: &str, ms: f64) {
        record_into(&mut self.systems, &mut self.recent.systems, name, ms);
    }

    /// Record `ms` of GPU time for the render passes of kind `name` this frame.
    pub fn record_gpu_pass(&mut self, name: &str, ms: f64) {
        record_into(&mut self.gpu_passes, &mut self.recent.gpu_passes, name, ms);
    }

    /// The stats over each metric's newest `n` samples (at most [`RECENT`]) instead
    /// of the whole session: `frames` and `drawn_frames` are capped at `n` too. A
    /// metric not recorded within them is absent. The result keeps no history.
    pub fn window(&self, n: usize) -> FrameStats {
        let n = n.min(RECENT);
        let cap = n as u64;
        FrameStats {
            frames: self.frames.min(cap),
            drawn_frames: self.drawn_frames.min(cap),
            metrics: fold_recent(&self.recent.metrics, n),
            systems: fold_recent(&self.recent.systems, n),
            gpu_passes: fold_recent(&self.recent.gpu_passes, n),
            recent: Recent::default(),
        }
    }

    /// One metric's summary, if it was ever recorded.
    pub fn get(&self, key: &str) -> Option<&Series> {
        self.metrics.get(key)
    }

    /// Check a budget: metric `key` must never have exceeded `limit` on any recorded
    /// frame. `Ok` carries a pass line and `Err` a failure line naming the measured
    /// worst frame. The pass line holds no measured value, so a passing timing budget
    /// leaves `results.json` byte-identical between runs.
    pub fn check_budget(&self, key: &str, limit: f64) -> Result<String, String> {
        let head = format!("budget {key} <= {limit}");
        match self.get(key) {
            Some(s) if s.max <= limit => Ok(head),
            Some(s) => Err(format!(
                "{head} exceeded: worst frame {:.3}, avg {:.3} over {} frames",
                s.max, s.avg, s.samples
            )),
            None => Err(format!(
                "{head}: `{key}` was never measured (known: {})",
                self.metrics.keys().cloned().collect::<Vec<_>>().join(", ")
            )),
        }
    }

    /// The stats as JSON: `{ frames, metrics: {key: series}, systems: {…},
    /// gpu_passes: {…} }`. With `timings` false only the deterministic counts are
    /// kept (no `_ms` or `_us` metric, no per-system or per-pass times), which is what a
    /// replay-stable `results.json` can carry. `gpu_passes` is absent when empty.
    pub fn to_json(&self, timings: bool) -> Value {
        let metrics: BTreeMap<&String, &Series> = self
            .metrics
            .iter()
            .filter(|(k, _)| timings || !is_timing(k))
            .collect();
        let mut out = json!({ "frames": self.frames, "metrics": metrics });
        if timings {
            out["systems"] = json!(self.systems);
            if !self.gpu_passes.is_empty() {
                out["gpu_passes"] = json!(self.gpu_passes);
            }
        }
        out
    }
}

fn record_into(
    map: &mut BTreeMap<String, Series>,
    recent: &mut BTreeMap<String, VecDeque<f64>>,
    key: &str,
    value: f64,
) {
    match map.get_mut(key) {
        Some(series) => series.record(value),
        None => map.entry(key.to_string()).or_default().record(value),
    }
    let values = match recent.get_mut(key) {
        Some(values) => values,
        None => recent.entry(key.to_string()).or_default(),
    };
    if values.len() == RECENT {
        values.pop_front();
    }
    values.push_back(value);
}

/// Each key's newest `n` values summarised as a fresh [`Series`]; a key with none
/// (`n` is 0) is left out.
fn fold_recent(recent: &BTreeMap<String, VecDeque<f64>>, n: usize) -> BTreeMap<String, Series> {
    let mut out = BTreeMap::new();
    for (key, values) in recent {
        let skip = values.len().saturating_sub(n);
        let mut series = Series::default();
        values.iter().skip(skip).for_each(|v| series.record(*v));
        if series.samples > 0 {
            out.insert(key.clone(), series);
        }
    }
    out
}

#[cfg(test)]
#[path = "frame_stats_tests.rs"]
mod frame_stats_tests;
