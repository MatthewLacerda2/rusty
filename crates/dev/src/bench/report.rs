//! The bench report: per-frame samples summarised as average and 95th percentile,
//! printed as a short table with each row's change against the previous run.
//! The summaries are pure; [`read`] and [`write`] keep the report in the out dir.
//!
//! stored-format: `bench.json`, the last run's report the next one compares against

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

use rusty_core::project::EngineStamp;

use super::engine::engine_line;
use serde::{Deserialize, Serialize};

/// One metric over the measured frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub avg: f64,
    pub p95: f64,
    /// A counter the printed report shows only when it changed against the last
    /// run ([`Samples::push_quiet`]); the report file holds it always.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
}

impl Summary {
    /// A loud row's summary: `avg` and `p95`, not [`quiet`](Self::quiet).
    pub fn new(avg: f64, p95: f64) -> Self {
        Self {
            avg,
            p95,
            quiet: false,
        }
    }

    /// The mean and the nearest-rank 95th percentile of `values` (zero when empty).
    pub fn of(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self::default();
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let rank = (0.95 * sorted.len() as f64).ceil() as usize;
        Self {
            avg: values.iter().sum::<f64>() / values.len() as f64,
            p95: sorted[rank.clamp(1, sorted.len()) - 1],
            quiet: false,
        }
    }
}

/// A finished run: what was measured, and each metric's summary in report order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub frames: u32,
    /// `(metric, summary)` in the order rows print — stable, so reports diff.
    pub metrics: Vec<(String, Summary)>,
    /// Why some rows should be read with care (a CPU clock that moved, #1719),
    /// printed under the table.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// Which engine measured it (`core::project::EngineStamp`): a "before" taken
    /// with another tree's shaders says so in its own report.
    #[serde(default)]
    pub engine: Option<EngineStamp>,
}

/// Per-frame samples, kept in the order metrics were first seen.
#[derive(Default)]
pub struct Samples {
    order: Vec<String>,
    values: BTreeMap<String, Vec<f64>>,
    /// The metrics pushed by [`Samples::push_quiet`].
    quiet: BTreeSet<String>,
    frames: u32,
}

impl Samples {
    /// Record one frame's `value` for `metric`.
    pub fn push(&mut self, metric: &str, value: f64) {
        if !self.values.contains_key(metric) {
            self.order.push(metric.to_string());
        }
        self.values
            .entry(metric.to_string())
            .or_default()
            .push(value);
    }

    /// Record one frame's `value` of a counter the printed report shows only when
    /// it changed (`bench::sample`): its row is [`Summary::quiet`].
    pub fn push_quiet(&mut self, metric: &str, value: f64) {
        self.push(metric, value);
        if !self.quiet.contains(metric) {
            self.quiet.insert(metric.to_string());
        }
    }

    /// Record a frame's parts of `stage` (a row pushed this frame), each named
    /// `<stage>.<part>`. Once a frame ran the stage, every part any of its frames
    /// lapped is sampled, zero when absent (a Mask submit on a frame with no Mask
    /// drawn), and a part first lapped late is zero on the stage's earlier frames:
    /// each part averages over its stage's frames, so the parts sum to it.
    pub fn push_stage_parts(
        &mut self,
        stage: &str,
        parts: impl IntoIterator<Item = (String, f64)>,
    ) {
        let ran = self.values(stage).len();
        let prefix = format!("{stage}.");
        let mut absent: Vec<String> = self
            .order
            .iter()
            .filter(|m| m.starts_with(&prefix))
            .cloned()
            .collect();
        for (part, ms) in parts {
            if !self.values.contains_key(&part) {
                for _ in 1..ran {
                    self.push(&part, 0.0);
                }
            }
            self.push(&part, ms);
            absent.retain(|m| *m != part);
        }
        for part in absent {
            self.push(&part, 0.0);
        }
    }

    /// Every frame's value of `metric`, in order (empty when never pushed).
    pub fn values(&self, metric: &str) -> &[f64] {
        self.values.get(metric).map_or(&[], Vec::as_slice)
    }

    /// Count one measured frame.
    pub fn end_frame(&mut self) {
        self.frames += 1;
    }

    pub fn report(&self) -> Report {
        let metrics = self
            .order
            .iter()
            .map(|m| {
                let quiet = self.quiet.contains(m);
                (
                    m.clone(),
                    Summary {
                        quiet,
                        ..Summary::of(&self.values[m])
                    },
                )
            })
            .collect();
        Report {
            frames: self.frames,
            metrics,
            ..Report::default()
        }
    }
}

/// The warning for a kept report that times the tick another way, by which rows
/// each has (`has`), so a sim change reads in its `cpu.<system>` rows:
/// - from before `cpu_ms` timed the whole tick (#2342): the kept one has a
///   `cpu_ms` and no `cpu.flush_bounds`, the current one that row, and `cpu_ms`
///   and `cpu.other` grew by what was hidden;
/// - from before the stats' own bookkeeping left the tick (#2445): the kept one
///   has `cpu.flush_bounds` and no `probe_ms`, the current one that row, and
///   `cpu_ms` and `cpu.other` shrank by about what `probe_ms` now holds.
pub fn tick_timed_since(now: impl Fn(&str) -> bool, kept: impl Fn(&str) -> bool) -> Option<String> {
    let old = kept("cpu_ms") && !kept("cpu.flush_bounds");
    if old && now("cpu.flush_bounds") {
        return Some(
            "the kept report predates `cpu_ms` timing the whole tick (it has no \
             cpu.flush_bounds row): cpu_ms and cpu.other now hold the tick's work outside \
             its systems, so compare the cpu.<system> rows"
                .to_string(),
        );
    }
    let counted = kept("cpu.flush_bounds") && !kept("probe_ms");
    (counted && now("probe_ms")).then(|| {
        "the kept report counts the stats' own bookkeeping in its tick (it has no \
         probe_ms row): cpu_ms and cpu.other now leave it out, lower by about probe_ms, \
         so compare the cpu.<system> rows"
            .to_string()
    })
}

impl Report {
    /// Whether this report has a row for `metric`.
    pub fn has(&self, metric: &str) -> bool {
        self.metrics.iter().any(|(k, _)| k == metric)
    }

    /// The table the terminal shows: one row per metric, with the change in its
    /// average against `previous` where that run measured it too. A quiet counter
    /// ([`Summary::quiet`]) prints only when it changed against `previous`, or reads
    /// `absent in last` where `previous` lacks it; one line counts the rest.
    pub fn render(&self, previous: Option<&Report>) -> String {
        let before: BTreeMap<&str, &Summary> = previous
            .map(|p| p.metrics.iter().map(|(k, s)| (k.as_str(), s)).collect())
            .unwrap_or_default();
        let mut out = format!("bench: {} frames\n", self.frames);
        let _ = writeln!(
            out,
            "{:<22} {:>12} {:>12}  vs last avg",
            "metric", "avg", "p95"
        );
        let mut unshown = 0;
        for (metric, s) in &self.metrics {
            let last = before.get(metric.as_str());
            if s.quiet && !quiet_moved(previous.is_some(), last.map(|b| b.avg), s.avg) {
                unshown += 1;
                continue;
            }
            let delta = match last {
                Some(b) => change(b.avg, s.avg),
                None if s.quiet => "absent in last".to_string(),
                None => String::new(),
            };
            let _ = writeln!(
                out,
                "{metric:<22} {:>12} {:>12}  {delta}",
                num(s.avg),
                num(s.p95)
            );
        }
        out.push_str(&unshown_line(unshown, previous.is_some(), "bench.json"));
        if let Some(engine) = &self.engine {
            out.push_str(&engine_line(
                engine,
                previous.and_then(|p| p.engine.as_ref()),
            ));
        }
        for warning in &self.warnings {
            let _ = writeln!(out, "warning: {warning}");
        }
        if previous.is_none() {
            out.push_str("(no previous report: the next run prints its change against this one)\n");
        }
        out
    }
}

/// Whether a quiet counter at `avg` prints against a kept run (`kept`) whose
/// average of it was `last`: it moved, or that run lacks it. With no kept run,
/// nothing to have moved against: it does not.
pub(super) fn quiet_moved(kept: bool, last: Option<f64>, avg: f64) -> bool {
    kept && last != Some(avg)
}

/// The line counting the `unshown` quiet counters a table left out (none: no
/// line): unchanged since the kept run, or with none, kept for the next to
/// compare against; all of them are in `file`.
pub(super) fn unshown_line(unshown: usize, kept: bool, file: &str) -> String {
    match (unshown, kept) {
        (0, _) => String::new(),
        (n, true) => format!("({n} other counters unchanged since the last run: in {file})\n"),
        (n, false) => format!("({n} other counters in {file}: each prints once it changes)\n"),
    }
}

/// A value at a precision that suits its size: ms to the microsecond, counts whole.
pub(super) fn num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.3}")
    }
}

/// `+0.123 (+4.5%)`; `=` when unchanged.
pub(super) fn change(before: f64, after: f64) -> String {
    let d = after - before;
    if d == 0.0 {
        return "=".to_string();
    }
    let pct = if before == 0.0 {
        String::new()
    } else {
        format!(" ({:+.1}%)", d / before * 100.0)
    };
    format!("{d:+.3}{pct}")
}

/// The last run's report, if `out_dir` holds a readable one.
pub(super) fn read(out_dir: &Path) -> Option<Report> {
    let json = std::fs::read_to_string(out_dir.join("bench.json")).ok()?;
    serde_json::from_str(&json).ok()
}

pub(super) fn write(out_dir: &Path, report: &Report, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let json = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    // lint: allow(asset-write) a bench report, not read by a path-keyed cache
    std::fs::write(out_dir.join("bench.json"), json)?;
    // lint: allow(asset-write) a bench report, not read by a path-keyed cache
    std::fs::write(out_dir.join("bench.txt"), text)
}

#[cfg(test)]
#[path = "quiet_tests.rs"]
mod quiet_tests;
#[cfg(test)]
#[path = "report_tests.rs"]
mod report_tests;
