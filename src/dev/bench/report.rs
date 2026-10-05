//! The bench report: per-frame samples summarised as average and 95th percentile,
//! printed as a short table with each row's change against the previous run.
//! Pure — the I/O (reading and writing `bench.json`) is the caller's.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};

/// One metric over the measured frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub avg: f64,
    pub p95: f64,
}

impl Summary {
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
        }
    }
}

/// A finished run: what was measured, and each metric's summary in report order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub frames: u32,
    /// `(metric, summary)` in the order rows print — stable, so reports diff.
    pub metrics: Vec<(String, Summary)>,
}

/// Per-frame samples, kept in the order metrics were first seen.
#[derive(Default)]
pub struct Samples {
    order: Vec<String>,
    values: BTreeMap<String, Vec<f64>>,
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

    /// Count one measured frame.
    pub fn end_frame(&mut self) {
        self.frames += 1;
    }

    pub fn report(&self) -> Report {
        let metrics = self
            .order
            .iter()
            .map(|m| (m.clone(), Summary::of(&self.values[m])))
            .collect();
        Report {
            frames: self.frames,
            metrics,
        }
    }
}

impl Report {
    /// The table the terminal shows: one row per metric, with the change in its
    /// average against `previous` where that run measured it too.
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
        for (metric, s) in &self.metrics {
            let delta = before
                .get(metric.as_str())
                .map_or(String::new(), |b| change(b.avg, s.avg));
            let _ = writeln!(
                out,
                "{metric:<22} {:>12} {:>12}  {delta}",
                num(s.avg),
                num(s.p95)
            );
        }
        if previous.is_none() {
            out.push_str("(no previous report: the next run prints its change against this one)\n");
        }
        out
    }
}

/// A value at a precision that suits its size: ms to the microsecond, counts whole.
fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() >= 1.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.3}")
    }
}

/// `+0.123 (+4.5%)`; `=` when unchanged.
fn change(before: f64, after: f64) -> String {
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

#[cfg(test)]
#[path = "report_tests.rs"]
mod report_tests;
