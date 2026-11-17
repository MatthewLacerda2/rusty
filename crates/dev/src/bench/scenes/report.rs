//! One scene's bench report: every `Harness.Bench` row aggregated over the scene's
//! viewpoints (the average of their averages, the worst p95 and the viewpoint that
//! gave it), its load time and memory, printed with each row's change against the
//! scene's last run; and the one-table summary over every scene of a run.
//!
//! stored-format: `<out>/<project>/<scene>.json`, the last run's report the next one compares against

use std::fmt::Write;
use std::path::{Path, PathBuf};

use rusty_core::project::EngineStamp;
use serde::{Deserialize, Serialize};

use super::super::report::{change, engine_line, num};
use super::super::{Report, Summary};

/// One row over a scene's viewpoints.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub metric: String,
    /// The average of the viewpoints' averages.
    pub avg: f64,
    /// The worst viewpoint's p95.
    pub p95: f64,
    /// The viewpoint the worst p95 came from (empty for a one-off row: load, memory).
    pub worst: String,
}

/// A scene's finished bench.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneReport {
    /// The scene's name: its file stem under `assets/scenes/`.
    pub scene: String,
    /// The viewpoints in the order they were measured.
    pub views: Vec<String>,
    /// Frames measured at each viewpoint.
    pub frames: u32,
    /// The run's render size.
    pub size: (u32, u32),
    pub rows: Vec<Row>,
    /// Which source `gpu_mb` came from (`rusty_render::GpuMemory::source`), if any.
    pub gpu_mb_source: Option<String>,
    pub warnings: Vec<String>,
    pub engine: Option<EngineStamp>,
}

impl SceneReport {
    /// The row named `metric`, if the run measured it.
    pub fn row(&self, metric: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.metric == metric)
    }

    /// Add a one-off row (load time, memory): avg and p95 are the value.
    pub fn push_value(&mut self, metric: &str, value: f64) {
        self.rows.push(Row {
            metric: metric.into(),
            avg: value,
            p95: value,
            worst: String::new(),
        });
    }

    /// The table the terminal shows, with each row's average against `previous`.
    pub fn render(&self, previous: Option<&SceneReport>) -> String {
        let (w, h) = self.size;
        let mut out = format!(
            "bench-scenes: {} — {} viewpoints x {} frames at {w}x{h}\n",
            self.scene,
            self.views.len(),
            self.frames
        );
        let _ = writeln!(
            out,
            "{:<22} {:>12} {:>12}  {:<18} vs last avg",
            "metric", "avg", "worst p95", "worst at"
        );
        for row in &self.rows {
            let delta = previous
                .and_then(|p| p.row(&row.metric))
                .map_or(String::new(), |b| change(b.avg, row.avg));
            let _ = writeln!(
                out,
                "{:<22} {:>12} {:>12}  {:<18} {delta}",
                row.metric,
                num(row.avg),
                num(row.p95),
                row.worst
            );
        }
        if let Some(engine) = &self.engine {
            out.push_str(&engine_line(
                engine,
                previous.and_then(|p| p.engine.as_ref()),
            ));
        }
        for warning in &self.warnings {
            let _ = writeln!(out, "warning: {warning}");
        }
        out.push_str(&self.notes(previous.is_none()));
        out
    }

    /// The lines under the table that say what the run could not measure.
    fn notes(&self, first: bool) -> String {
        let mut out = String::new();
        let notes = [
            (
                self.row("renderer_ms").is_none(),
                "(no GPU or software adapter: nothing was rendered)",
            ),
            (
                self.row("renderer_ms").is_some() && self.row("gpu_ms").is_none(),
                "(this adapter has no timestamp queries: no GPU times)",
            ),
            (
                self.row("gpu_mb").is_none(),
                "(this backend reports no GPU memory: no gpu_mb)",
            ),
            (
                first,
                "(no previous report: the next run prints its change against this one)",
            ),
        ];
        for (_, note) in notes.iter().filter(|(on, _)| *on) {
            let _ = writeln!(out, "{note}");
        }
        if let Some(source) = &self.gpu_mb_source {
            let _ = writeln!(out, "(gpu_mb: {source})");
        }
        out
    }
}

/// Fold the viewpoints' reports, labelled, into rows: metrics in the order they
/// were first seen, each the average of the averages and the worst p95.
pub fn aggregate(reports: &[(String, Report)]) -> Vec<Row> {
    let mut rows: Vec<(Row, u32)> = Vec::new();
    for (label, report) in reports {
        for (metric, s) in &report.metrics {
            let at = match rows.iter().position(|(r, _)| &r.metric == metric) {
                Some(at) => at,
                None => {
                    rows.push((first(metric, label, s), 0));
                    rows.len() - 1
                }
            };
            let (row, n) = &mut rows[at];
            row.avg += s.avg;
            *n += 1;
            if s.p95 > row.p95 {
                (row.p95, row.worst) = (s.p95, label.clone());
            }
        }
    }
    rows.into_iter()
        .map(|(mut row, n)| {
            row.avg /= f64::from(n.max(1));
            row
        })
        .collect()
}

fn first(metric: &str, label: &str, s: &Summary) -> Row {
    Row {
        metric: metric.into(),
        avg: 0.0,
        p95: s.p95,
        worst: label.into(),
    }
}

/// The rows the run's summary shows for each scene, in order.
const SUMMARY: [&str; 5] = ["frame_ms", "gpu_ms", "load_ms", "rss_peak_mb", "gpu_mb"];

/// One line per scene: its frame, GPU and load times (averages) and its memory.
pub fn summary(reports: &[SceneReport]) -> String {
    let mut out = format!("bench-scenes: {} scenes\n{:<20}", reports.len(), "scene");
    for metric in SUMMARY {
        let _ = write!(out, " {metric:>12}");
    }
    out.push('\n');
    for report in reports {
        let _ = write!(out, "{:<20}", report.scene);
        for metric in SUMMARY {
            let value = report.row(metric).map_or("-".into(), |r| num(r.avg));
            let _ = write!(out, " {value:>12}");
        }
        out.push('\n');
    }
    out
}

/// Where `scene`'s report is kept under the run's `out_dir`.
pub fn path(out_dir: &Path, scene: &str) -> PathBuf {
    out_dir.join(format!("{scene}.json"))
}

/// The scene's last report, if `out_dir` holds a readable one.
pub fn read(out_dir: &Path, scene: &str) -> Option<SceneReport> {
    let json = std::fs::read_to_string(path(out_dir, scene)).ok()?;
    serde_json::from_str(&json).ok()
}

/// Keep `report` (and the table printed for it) in `out_dir`.
pub fn write(out_dir: &Path, report: &SceneReport, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let json = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    // lint: allow(asset-write) a bench report, not read by a path-keyed cache
    std::fs::write(path(out_dir, &report.scene), json)?;
    let txt = out_dir.join(format!("{}.txt", report.scene));
    // lint: allow(asset-write) a bench report, not read by a path-keyed cache
    std::fs::write(txt, text)
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
