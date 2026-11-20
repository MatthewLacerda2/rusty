//! crates/core/src/frame_stats/mod.rs — per-frame perf counters and timings for agents (#433).
//!
//! An agent never *feels* the frame rate, so it needs numbers: CPU milliseconds per
//! schedule stage and per system, render counters (draw calls, triangles, culled
//! entities, dropped lights) and world counters (entities, bodies, agents, scripts).
//! [`FrameStats`] is where those numbers land — a flat, name-keyed set of running
//! [`Series`] (last / min / avg / max over the frames recorded) that `Debug.Stats()`,
//! the harness's `results.json` and scenario budgets all read.
//!
//! **Now, not the session.** Beside each series its newest [`RECENT`] samples are
//! kept, each stamped with the tick and drawn frame it was recorded at, so
//! [`FrameStats::window`] can summarise only the newest frames: what
//! `Debug.Stats{ window = n }` and the Game view's Stats overlay read. A metric
//! that stopped being recorded (a pass turned off, a load long ago) leaves the
//! window once its newest stamp is older than it. The session's series take Play
//! alone; the ring takes every drawn frame, edit mode too.
//!
//! This is plain data. **Nothing here reads a clock**: the timings are measured by the
//! dev layer (`dev::stats`) around each system and written in, and the sim never reads
//! them back, so the determinism rule is untouched. A key [`is_timing`] names (one
//! ending in `_ms` or `_us`, and `fps`) is wall-clock and differs run to run; every
//! other key is a count ([`is_counter`]) and is deterministic for a deterministic
//! run — [`FrameStats::to_json`] can split the two.

use std::collections::{BTreeMap, VecDeque};

mod keys;
mod recent;

pub use keys::{is_counter, is_timing};

use recent::{bounded, fold_recent, push, Recent, Stamp};
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

/// How many of each metric's newest samples are kept for [`FrameStats::window`]:
/// ten seconds of a 60 Hz tick or window. A wider window is clamped to it.
pub const RECENT: usize = 600;

/// Every metric recorded since the stats were last reset (a Play session).
///
/// Two kinds of sample land here: a sim tick's ([`FrameStats::record`],
/// [`FrameStats::record_system`], closed by [`FrameStats::end_tick`]) and a drawn
/// frame's ([`FrameStats::record_frame`]). The session's series take every tick and
/// the drawn frames recorded in session; the recent ring takes both kinds always.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Sim frames (ticks) recorded.
    pub frames: u64,
    /// Frames drawn and recorded with their render counters in session: windowed
    /// Play frames, or headless frames something rendered. Not `frames`: a windowed
    /// frame runs zero or several ticks, and a headless tick need not draw.
    pub drawn_frames: u64,
    /// Stage timings, render counters and world counters, by name.
    pub metrics: BTreeMap<String, Series>,
    /// Per-system CPU milliseconds, by system name — diagnostic, not budgetable.
    pub systems: BTreeMap<String, Series>,
    /// The newest tick's CPU milliseconds per system, summed by name, in the order
    /// the schedule ran them: every registered system, so the bench reports each
    /// one (`cpu.<system>`) with no list of its own.
    pub tick_systems: Vec<(&'static str, f64)>,
    /// A tick closed since the last system was recorded: the next one starts
    /// [`Self::tick_systems`] afresh.
    tick_closed: bool,
    /// GPU milliseconds per render-pass kind, by its stable name (#835). Empty where
    /// the adapter has no timestamps — absent, never zero.
    pub gpu_passes: BTreeMap<String, Series>,
    /// Each series' newest samples, stamped, oldest first, at most [`RECENT`].
    recent: Recent,
}

/// One drawn frame being recorded ([`FrameStats::record_frame`]).
pub struct DrawnFrame<'a> {
    stats: &'a mut FrameStats,
    in_session: bool,
    rendered: bool,
}

impl DrawnFrame<'_> {
    /// Record `value` for metric `key` this frame.
    pub fn record(&mut self, key: &str, value: f64) {
        let at = self.stats.recent.stamp(false);
        let session = self.in_session.then_some(&mut self.stats.metrics);
        push(session, &mut self.stats.recent.metrics, at, key, value);
    }

    /// Record `ms` of GPU time for the render passes of kind `name` this frame.
    pub fn record_gpu_pass(&mut self, name: &str, ms: f64) {
        let at = self.stats.recent.stamp(false);
        let session = self.in_session.then_some(&mut self.stats.gpu_passes);
        push(session, &mut self.stats.recent.gpu_passes, at, name, ms);
    }

    /// Mark the frame as one the renderer drew: it counts as a drawn frame.
    pub fn rendered(&mut self) {
        self.rendered = true;
    }
}

impl FrameStats {
    /// Record `value` for metric `key` this tick.
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

    /// Record `ms` of CPU time for the system named `name` this tick (a
    /// schedule's names are static: the tick's list allocates none).
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

    /// Close a tick: one more of `frames`, and of the ring's tick clock.
    pub fn end_tick(&mut self) {
        let at = self.recent.stamp(true);
        bounded(&mut self.recent.tick_marks, at);
        self.tick_closed = true;
        self.frames += 1;
        self.recent.ticks += 1;
    }

    /// Record one drawn frame through `f`, then close it. With `in_session` false
    /// (an edit-mode frame) its samples go to the recent ring alone: the session
    /// keeps what Play recorded, and a window still reads the frame.
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

    /// The newest recorded frame ended: record `value` as its `key`, stamped at
    /// that frame and in the session when that frame was, for what is known only
    /// once the next frame starts (its wall time). Once per frame: a second call,
    /// or one with no frame recorded since the stats were made, records nothing,
    /// so a window's newest frame has no such sample yet (absent, never zero).
    pub fn frame_ended(&mut self, key: &str, value: f64) {
        if let Some((at, in_session)) = self.recent.open.take() {
            let session = in_session.then_some(&mut self.metrics);
            push(session, &mut self.recent.metrics, at, key, value);
        }
    }

    /// The stats over the newest `n` frames (at most [`RECENT`] samples a metric)
    /// instead of the whole session. A sample is in when it was recorded within the
    /// last `n` frames the stats recorded; a tick's sample within the last `n`
    /// ticks too. `frames` and `drawn_frames` count the ticks and rendered frames
    /// inside. A metric with no sample inside is absent, so a pass that stopped
    /// running leaves. The result keeps no history.
    pub fn window(&self, n: usize) -> FrameStats {
        let n = n.min(RECENT) as u64;
        self.folded(n, |at| self.recent.within(at, n))
    }

    /// The newest recorded frame alone, every tick it ran included (where
    /// [`window`](Self::window)`(1)` keeps only the newest tick): what the frame log
    /// writes per drawn frame (#2206). A tick metric's `samples` counts its ticks,
    /// so `avg * samples` is the frame's total.
    pub fn last_frame(&self) -> FrameStats {
        self.folded(1, |at| !self.recent.past_edge(at, 1))
    }

    /// The samples within the newest `n` frames that `keep` also takes, folded.
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

    /// The stats as JSON: `{ frames, drawn_frames, metrics: {key: series}, systems:
    /// {…}, gpu_passes: {…} }`. With `timings` false only the deterministic counts are
    /// kept (no metric [`is_timing`] names, no per-system or per-pass times), which is what a
    /// replay-stable `results.json` can carry. `gpu_passes` is absent when empty.
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
