//! src/core/frame_stats.rs — per-frame perf counters and timings for agents (#433).
//!
//! An agent never *feels* the frame rate, so it needs numbers: CPU milliseconds per
//! schedule stage and per system, render counters (draw calls, triangles, culled
//! entities, dropped lights) and world counters (entities, bodies, agents, scripts).
//! [`FrameStats`] is where those numbers land — a flat, name-keyed set of running
//! [`Series`] (last / min / avg / max over the frames recorded) that `Debug.Stats()`,
//! the harness's `results.json` and scenario budgets all read.
//!
//! This is plain data. **Nothing here reads a clock**: the timings are measured by the
//! dev layer (`dev::stats`) around each system and written in, and the sim never reads
//! them back, so the determinism rule is untouched. Keys ending in `_ms` are
//! wall-clock timings and differ run to run; every other key is a count and is
//! deterministic for a deterministic run — [`FrameStats::to_json`] can split the two.

use std::collections::BTreeMap;

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

/// Every metric recorded since the stats were last reset (a Play session).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Sim frames recorded.
    pub frames: u64,
    /// Stage timings, render counters and world counters, by name.
    pub metrics: BTreeMap<String, Series>,
    /// Per-system CPU milliseconds, by system name — diagnostic, not budgetable.
    pub systems: BTreeMap<String, Series>,
}

/// Whether `key` names a wall-clock timing (non-deterministic) rather than a count.
pub fn is_timing(key: &str) -> bool {
    key.ends_with("_ms")
}

impl FrameStats {
    /// Record `value` for metric `key` this frame.
    pub fn record(&mut self, key: &str, value: f64) {
        record_into(&mut self.metrics, key, value);
    }

    /// Record `ms` of CPU time for the system named `name` this frame.
    pub fn record_system(&mut self, name: &str, ms: f64) {
        record_into(&mut self.systems, name, ms);
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

    /// The stats as JSON: `{ frames, metrics: {key: series}, systems: {…} }`. With
    /// `timings` false only the deterministic counts are kept (no `_ms` metric, no
    /// per-system times), which is what a replay-stable `results.json` can carry.
    pub fn to_json(&self, timings: bool) -> Value {
        let metrics: BTreeMap<&String, &Series> = self
            .metrics
            .iter()
            .filter(|(k, _)| timings || !is_timing(k))
            .collect();
        let mut out = json!({ "frames": self.frames, "metrics": metrics });
        if timings {
            out["systems"] = json!(self.systems);
        }
        out
    }
}

fn record_into(map: &mut BTreeMap<String, Series>, key: &str, value: f64) {
    match map.get_mut(key) {
        Some(series) => series.record(value),
        None => map.entry(key.to_string()).or_default().record(value),
    }
}

#[cfg(test)]
#[path = "frame_stats_tests.rs"]
mod frame_stats_tests;
