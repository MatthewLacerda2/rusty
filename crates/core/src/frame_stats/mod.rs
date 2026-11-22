//! Nothing here reads a clock: the dev layer measures timings and writes them in, and
//! the sim never reads them back. Keys [`is_timing`] names are wall-clock; the rest
//! are counts, deterministic for a deterministic run.

use std::collections::{BTreeMap, VecDeque};

mod keys;
mod recent;

pub use keys::{is_counter, is_timing};

use recent::{bounded, fold_recent, push, Recent, Stamp};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Series {
    pub last: f64,
    pub min: f64,
    pub avg: f64,
    pub max: f64,
    pub samples: u64,
}

impl Series {
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

pub const RECENT: usize = 600;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    pub frames: u64,
    pub drawn_frames: u64,
    pub metrics: BTreeMap<String, Series>,
    pub systems: BTreeMap<String, Series>,
    pub tick_systems: Vec<(&'static str, f64)>,
    tick_closed: bool,
    pub gpu_passes: BTreeMap<String, Series>,
    recent: Recent,
}

pub struct DrawnFrame<'a> {
    stats: &'a mut FrameStats,
    in_session: bool,
    rendered: bool,
}

impl DrawnFrame<'_> {
    pub fn record(&mut self, key: &str, value: f64) {
        let at = self.stats.recent.stamp(false);
        let session = self.in_session.then_some(&mut self.stats.metrics);
        push(session, &mut self.stats.recent.metrics, at, key, value);
    }

    pub fn record_gpu_pass(&mut self, name: &str, ms: f64) {
        let at = self.stats.recent.stamp(false);
        let session = self.in_session.then_some(&mut self.stats.gpu_passes);
        push(session, &mut self.stats.recent.gpu_passes, at, name, ms);
    }

    pub fn rendered(&mut self) {
        self.rendered = true;
    }
}

impl FrameStats {
    pub fn record(&mut self, key: &str, value: f64) {
        let at = self.recent.stamp(true);
        push(
            Some(&mut self.metrics),
            &mut self.recent.metrics,
            at,
            key,
            value,
        );
    }

    pub fn record_system(&mut self, name: &'static str, ms: f64) {
        if std::mem::take(&mut self.tick_closed) {
            self.tick_systems.clear();
        }
        match self.tick_systems.iter_mut().find(|(n, _)| *n == name) {
            Some((_, sum)) => *sum += ms,
            None => self.tick_systems.push((name, ms)),
        }
        let at = self.recent.stamp(true);
        push(
            Some(&mut self.systems),
            &mut self.recent.systems,
            at,
            name,
            ms,
        );
    }

    pub fn end_tick(&mut self) {
        let at = self.recent.stamp(true);
        bounded(&mut self.recent.tick_marks, at);
        self.tick_closed = true;
        self.frames += 1;
        self.recent.ticks += 1;
    }

    pub fn record_frame(&mut self, in_session: bool, f: impl FnOnce(&mut DrawnFrame<'_>)) {
        let opened = self.recent.stamp(false);
        let mut frame = DrawnFrame {
            stats: self,
            in_session,
            rendered: false,
        };
        f(&mut frame);
        if frame.rendered {
            let at = self.recent.stamp(false);
            bounded(&mut self.recent.drawn_marks, at);
            self.drawn_frames += u64::from(in_session);
        }
        self.recent.open = Some((opened, in_session));
        self.recent.frames += 1;
    }

    /// Records nothing on a second call per frame: a missing sample is absent, never zero.
    pub fn frame_ended(&mut self, key: &str, value: f64) {
        if let Some((at, in_session)) = self.recent.open.take() {
            let session = in_session.then_some(&mut self.metrics);
            push(session, &mut self.recent.metrics, at, key, value);
        }
    }

    /// A metric with no sample inside the newest `n` frames is absent.
    pub fn window(&self, n: usize) -> FrameStats {
        let n = n.min(RECENT) as u64;
        self.folded(n, |at| self.recent.within(at, n))
    }

    pub fn last_frame(&self) -> FrameStats {
        self.folded(1, |at| !self.recent.past_edge(at, 1))
    }

    fn folded(&self, n: u64, keep: impl Fn(Stamp) -> bool) -> FrameStats {
        let r = &self.recent;
        let count = |marks: &VecDeque<Stamp>| {
            let edge = marks.partition_point(|at| r.past_edge(*at, n));
            marks.range(edge..).filter(|at| keep(**at)).count()
        };
        FrameStats {
            frames: count(&r.tick_marks) as u64,
            drawn_frames: count(&r.drawn_marks) as u64,
            metrics: fold_recent(r, &r.metrics, n, &keep),
            systems: fold_recent(r, &r.systems, n, &keep),
            gpu_passes: fold_recent(r, &r.gpu_passes, n, &keep),
            tick_systems: Vec::new(),
            tick_closed: false,
            recent: Recent::default(),
        }
    }

    pub fn get(&self, key: &str) -> Option<&Series> {
        self.metrics.get(key)
    }

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

    pub fn to_json(&self, timings: bool) -> Value {
        let metrics: BTreeMap<&String, &Series> = self
            .metrics
            .iter()
            .filter(|(k, _)| timings || !is_timing(k))
            .collect();
        let mut out = json!({
            "frames": self.frames,
            "drawn_frames": self.drawn_frames,
            "metrics": metrics,
        });
        if timings {
            out["systems"] = json!(self.systems);
            if !self.gpu_passes.is_empty() {
                out["gpu_passes"] = json!(self.gpu_passes);
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod frame_stats_tests;
#[cfg(test)]
#[path = "window_tests.rs"]
mod window_tests;
