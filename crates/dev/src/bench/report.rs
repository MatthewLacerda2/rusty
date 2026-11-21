//! stored-format: `bench.json`, the last run's report the next one compares against

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

use rusty_core::project::EngineStamp;

use super::engine::engine_line;
use super::quiet::{quiet_moved, unshown_line};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub avg: f64,
    pub p95: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
}

impl Summary {
    pub fn new(avg: f64, p95: f64) -> Self {
        Self {
            avg,
            p95,
            quiet: false,
        }
    }

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub frames: u32,
    pub metrics: Vec<(String, Summary)>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub engine: Option<EngineStamp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<String>,
}

#[derive(Default)]
pub struct Samples {
    order: Vec<String>,
    values: BTreeMap<String, Vec<f64>>,
    quiet: BTreeSet<String>,
    frames: u32,
}

impl Samples {
    pub fn push(&mut self, metric: &str, value: f64) {
        if !self.values.contains_key(metric) {
            self.order.push(metric.to_string());
        }
        self.values
            .entry(metric.to_string())
            .or_default()
            .push(value);
    }

    pub fn push_quiet(&mut self, metric: &str, value: f64) {
        self.push(metric, value);
        if !self.quiet.contains(metric) {
            self.quiet.insert(metric.to_string());
        }
    }

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

    pub fn values(&self, metric: &str) -> &[f64] {
        self.values.get(metric).map_or(&[], Vec::as_slice)
    }

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
    pub fn has(&self, metric: &str) -> bool {
        self.metrics.iter().any(|(k, _)| k == metric)
    }

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
        let compared = previous.is_some_and(|p| !p.metrics.is_empty());
        let mut unshown = 0;
        for (metric, s) in &self.metrics {
            let last = before.get(metric.as_str());
            if s.quiet && !quiet_moved(compared, last.map(|b| b.avg), s.avg) {
                unshown += 1;
                continue;
            }
            let delta = match (previous, last) {
                (_, Some(b)) => change(b.avg, s.avg),
                (Some(_), None) => "absent on the last run".to_string(),
                (None, None) => String::new(),
            };
            let _ = writeln!(
                out,
                "{metric:<22} {:>12} {:>12}  {delta}",
                num(s.avg),
                num(s.p95)
            );
        }
        let against = compared.then_some("last");
        out.push_str(&unshown_line(unshown, against, "bench.json"));
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

pub(super) fn num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.3}")
    }
}

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
