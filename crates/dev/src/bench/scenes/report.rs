//! One scene's bench report: every `Harness.Bench` row folded over the scene's
//! viewpoints (`fold`: the average of their averages, the worst p95 and the
//! viewpoint that gave it), its load time and memory, printed with each row's
//! change against the scene's last run of the same shape; and the one-table
//! summary over every scene of a run.
//!
//! stored-format: `<out>/<project>-<hash>/<scene>.json`, the last run's report the next one compares against

use std::fmt::Write;
use std::path::{Path, PathBuf};

use rusty_core::project::EngineStamp;
use serde::{Deserialize, Serialize};

use super::super::clock::clock_drift;
use super::super::report::{change, engine_line, num};

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
    /// The scene's name: its path under `assets/scenes/`, without the extension.
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

    /// Whether `previous` benched what this run did: the same size, viewpoints
    /// and frames. Only then do their rows compare.
    pub fn same_shape(&self, previous: &SceneReport) -> bool {
        (self.size, &self.views, self.frames) == (previous.size, &previous.views, previous.frames)
    }

    /// What the comparison with `previous` should warn of: a run of another
    /// shape ([`Self::same_shape`]), whose rows get no change column, or a CPU
    /// clock that moved since it (`clock::cpu_drift`).
    pub fn against(&self, previous: Option<&SceneReport>) -> Vec<String> {
        let Some(previous) = previous else {
            return Vec::new();
        };
        if !self.same_shape(previous) {
            let mut what = Vec::new();
            if self.size != previous.size {
                let ((w, h), (pw, ph)) = (self.size, previous.size);
                what.push(format!("size {pw}x{ph} -> {w}x{h}"));
            }
            if self.views != previous.views {
                what.push(format!(
                    "viewpoints {} -> {}, or other ones",
                    previous.views.len(),
                    self.views.len()
                ));
            }
            if self.frames != previous.frames {
                what.push(format!("frames {} -> {}", previous.frames, self.frames));
            }
            return vec![format!(
                "the last run benched another shape ({}): no change column this time",
                what.join(", ")
            )];
        }
        let clock = |r: &SceneReport| r.row("cpu_clock_pct").map(|r| r.avg);
        clock(self)
            .zip(clock(previous))
            .and_then(|(now, before)| clock_drift(now, before))
            .into_iter()
            .collect()
    }

    /// The table the terminal shows, with each row's average against `previous`
    /// when it has the same shape ([`Self::same_shape`]).
    pub fn render(&self, previous: Option<&SceneReport>) -> String {
        let comparable = previous.filter(|p| self.same_shape(p));
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
            let delta = comparable
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
                self.row("gpu_ms").is_some(),
                "(gpu_ms, gpu.<pass>: ms at the reference clock; gpu_raw_ms as measured, \
                 at gpu_clock_pct of it)",
            ),
            (
                self.row("cpu_ms").is_some(),
                "(cpu_ms, cpu.<system>: ms at the reference CPU clock; frame_ms as measured, \
                 at cpu_clock_pct of it)",
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

/// The rows the run's summary shows for each scene, in order: `cpu_ms` is the
/// sim's time at the reference CPU clock, `frame_ms` as measured.
const SUMMARY: [&str; 6] = [
    "cpu_ms",
    "frame_ms",
    "gpu_ms",
    "load_ms",
    "rss_peak_mb",
    "gpu_mb",
];

/// One line per scene: its CPU, frame, GPU and load times (averages) and its memory.
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

/// Keep `report` (and the table printed for it) in `out_dir`, in the subfolders
/// a scene's name holds (`levels/a`).
pub fn write(out_dir: &Path, report: &SceneReport, text: &str) -> std::io::Result<()> {
    let json_path = path(out_dir, &report.scene);
    std::fs::create_dir_all(json_path.parent().unwrap_or(out_dir))?;
    let json = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    // lint: allow(asset-write) a bench report, not read by a path-keyed cache
    std::fs::write(json_path, json)?;
    let txt = out_dir.join(format!("{}.txt", report.scene));
    // lint: allow(asset-write) a bench report, not read by a path-keyed cache
    std::fs::write(txt, text)
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
