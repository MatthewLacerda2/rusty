//! One scene's bench report: every `Harness.Bench` row folded over the scene's
//! viewpoints and runs (`fold`: the average of their averages, the worst p95 and
//! the viewpoint that gave it, and the spread of the runs' averages), its load
//! time and memory, printed with each row's change against a run of the same
//! shape (`compare`: the scene's last one, or a kept baseline's); and the
//! one-table summary over every scene of a run.
//!
//! stored-format: `<out>/<project>-<hash>/<scene>.json`, the last run's report the next one compares against

use std::fmt::Write;
use std::path::{Path, PathBuf};

use rusty_core::project::EngineStamp;
use serde::{Deserialize, Serialize};

use super::super::report::{engine_line, num};
mod compare;
mod view_key;
pub use compare::{delta, spread};
pub use view_key::ViewKey;

/// The note a `PROFILE=1` run's report carries: its rows come from the
/// frame-pointer build, so a comparison with a release run says so.
pub const PROFILING_NOTE: &str = "(timed on the profiling build: release with frame pointers \
     forced on, for PROFILE=1; within the release build's spread where measured)";

/// One row over a scene's viewpoints and runs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub metric: String,
    /// The average over the runs of the average of the viewpoints' averages.
    pub avg: f64,
    /// The worst viewpoint's p95, over every run.
    pub p95: f64,
    /// The viewpoint the worst p95 came from (empty for a one-off row: load, memory).
    pub worst: String,
    /// The lowest and highest of the runs' averages: the run-to-run spread, which
    /// a change must leave to be read as one (`compare`). Equal to `avg` for a
    /// one-off row, or a run of one.
    pub lo: f64,
    pub hi: f64,
}

/// A scene's finished bench.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneReport {
    /// The scene's name: its path under `assets/scenes/`, without the extension.
    pub scene: String,
    /// The viewpoints in the order they were measured.
    pub views: Vec<ViewKey>,
    /// Frames measured at each viewpoint, each run.
    pub frames: u32,
    /// How many times every viewpoint was measured (`RUNS`).
    pub runs: u32,
    /// The frames really measured, over every viewpoint and run: fewer than
    /// `runs x views x frames` when the game quit during the bench.
    pub measured: u32,
    /// The kept report this one was compared with (`BEFORE=<dir>`), when not the
    /// scene's last run.
    pub baseline: Option<String>,
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
            lo: value,
            hi: value,
        });
    }

    /// Whether `previous` benched what this run did: the same size, viewpoints
    /// (each where it looks from, [`ViewKey`]) and frames. Only then do their
    /// rows compare; the number of runs may differ.
    pub fn same_shape(&self, previous: &SceneReport) -> bool {
        (self.size, &self.views, self.frames) == (previous.size, &previous.views, previous.frames)
    }

    /// The table the terminal shows, with each row's average against `previous`
    /// when it has the same shape ([`Self::same_shape`]): its change, or
    /// `unresolved` when the two runs' spreads overlap (`compare::delta`).
    pub fn render(&self, previous: Option<&SceneReport>) -> String {
        let comparable = previous.filter(|p| self.same_shape(p));
        let (w, h) = self.size;
        let runs = self.runs.max(1);
        let mut out = format!(
            "bench-scenes: {} — {} viewpoints x {} frames x {runs} runs at {w}x{h}\n",
            self.scene,
            self.views.len(),
            self.frames
        );
        let against = if self.baseline.is_some() {
            "before"
        } else {
            "last"
        };
        let _ = writeln!(
            out,
            "{:<26} {:>12} {:>19} {:>12}  {:<18} vs {against} avg",
            "metric", "avg", "spread", "worst p95", "worst at"
        );
        for row in &self.rows {
            let delta = comparable
                .and_then(|p| p.row(&row.metric))
                .map_or(String::new(), |b| delta(b, row));
            let _ = writeln!(
                out,
                "{:<26} {:>12} {:>19} {:>12}  {:<18} {delta}",
                row.metric,
                num(row.avg),
                spread(row),
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
                "(gpu_ms, gpu.<pass>, gpu.other: ms at the reference clock, gpu_ms the passes' \
                 busy sum and gpu.other untimed work inside a submit; gpu_between_submits_ms \
                 the GPU waiting on the CPU, as measured; gpu_raw_ms as measured, at \
                 gpu_clock_pct of the reference)",
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
                self.row("renderer_ms").is_some(),
                "(render.<stage>: the renderer's CPU time by stage, summing to renderer_ms; \
                 render.other is what no stage timed)",
            ),
            (
                self.row("render.occlusion.test").is_some(),
                "(render.occlusion.<part>: the Hi-Z stage's CPU parts, inside its row, in no sum)",
            ),
            (
                // Only with GPU timing: an untimed frame records the stage in 2.
                self.row("gpu.occlusion.select").is_some(),
                "(gpu.occlusion.<part>: its GPU parts, inside its row, in no sum; a timed frame \
                 records each in a pass of its own, so 1 or 2 more compute_passes than untimed)",
            ),
        ];
        let notes = notes.into_iter().chain(self.comparison_notes(first));
        for (_, note) in notes.filter(|(on, _)| *on) {
            let _ = writeln!(out, "{note}");
        }
        if let Some(source) = &self.gpu_mb_source {
            let _ = writeln!(out, "(gpu_mb: {source})");
        }
        if let Some(dir) = &self.baseline {
            let _ = writeln!(out, "(compared with the kept report in {dir})");
        }
        if super::profile::on_profiling_build() {
            let _ = writeln!(out, "{PROFILING_NOTE}");
        }
        out
    }
}

impl SceneReport {
    /// The notes on what the change column compares: the spread, or no earlier run.
    fn comparison_notes(&self, first: bool) -> [(bool, &'static str); 3] {
        [
            (
                self.runs.max(1) > 1,
                "(spread: the lowest to the highest run's average; a change inside both \
                 runs' spreads is unresolved)",
            ),
            (
                first && self.baseline.is_none(),
                "(no previous report: the next run prints its change against this one)",
            ),
            (
                first && self.baseline.is_some(),
                "(the baseline holds no report of this scene: no change column)",
            ),
        ]
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

/// One line per scene: its CPU, frame, GPU and load times (averages) and its
/// memory; then the run's `import_ms` (`IMPORT=1`), the project's, once.
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
    let import = reports
        .iter()
        .find_map(|r| Some((r.row("import_ms")?, &r.scene)));
    if let Some((row, scene)) = import {
        let ms = num(row.avg);
        let _ = writeln!(
            out,
            "import_ms {ms} (the project's cold import, loading {scene})"
        );
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

/// The scene's report in a kept baseline `dir` (`BEFORE=<dir>`): a copy of a
/// project's report folder (`<dir>/<scene>.json`), or a copy of the whole
/// `out/bench/scenes/` holding it under its project's key (`<dir>/<key>/`).
pub fn read_baseline(dir: &Path, key: &str, scene: &str) -> Option<SceneReport> {
    read(dir, scene).or_else(|| read(&dir.join(key), scene))
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
#[path = "tests.rs"]
mod tests;
