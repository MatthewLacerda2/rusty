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

use super::super::engine::engine_line;
use super::super::quiet::{quiet_moved, unshown_line};
use super::super::report::num;
mod compare;
mod notes;
mod view_key;
pub use compare::{delta, spread};
pub use view_key::ViewKey;

/// The note a `PROFILE=1` run's report carries: its rows come from the
/// frame-pointer build, so a comparison with a release run says so.
pub const PROFILING_NOTE: &str = "(timed on the profiling build: release with frame pointers \
     forced on, for PROFILE=1; within the release build's spread where measured)";

/// The line a `SCRIPT=1` run's table carries under its title.
pub const SCRIPT_NOTE: &str = "script verbs timed (SCRIPT=1): script.<verb> is a verb's own ms, \
     script.lua what of cpu.update_scripts neither a verb nor the timers (script.overhead) hold; \
     the timers cost time, so \
     cpu.update_scripts compares only with another SCRIPT=1 run\n";

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
    /// A quiet counter (`bench::Summary::quiet`): printed only when it changed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
}

impl Row {
    /// A one-off row of `value`: no viewpoint, no spread.
    pub fn value(metric: String, value: f64) -> Row {
        let (avg, p95, lo, hi) = (value, value, value, value);
        let worst = String::new();
        Row {
            metric,
            avg,
            p95,
            worst,
            lo,
            hi,
            quiet: false,
        }
    }
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
    /// How many times the scene was loaded, each in a process of its own
    /// (`LOAD_RUNS`): the first cold, the rest the `load_ms` rows' spread.
    #[serde(default)]
    pub loads: u32,
    /// The frames really measured, over every viewpoint and run: fewer than
    /// `runs x views x frames` when the game quit during the bench.
    pub measured: u32,
    /// The kept report this one was compared with (`BEFORE=<dir>`), when not the
    /// scene's last run.
    pub baseline: Option<String>,
    /// The run's render size.
    pub size: (u32, u32),
    /// The running quality preset's overrides for this run (`--set key=value`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<String>,
    /// A `SCRIPT=1` run (`bench::verbs`): its verbs were timed, which costs time,
    /// so it compares only with another such run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub script: bool,
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
        self.rows.push(Row::value(metric.into(), value));
    }

    /// The metric column's width: 30, or the longest row (a `script.` verb's).
    fn metric_width(&self) -> usize {
        self.rows
            .iter()
            .map(|r| r.metric.len())
            .fold(30, usize::max)
    }

    /// Whether `previous` benched what this run did: the same size, viewpoints
    /// (each where it looks from, [`ViewKey`]) and frames, both timing their
    /// verbs or neither (`script`). Only then do their rows compare; the number
    /// of runs may differ.
    pub fn same_shape(&self, previous: &SceneReport) -> bool {
        let shape = |r: &SceneReport| (r.size, r.views.clone(), r.frames, r.script);
        shape(self) == shape(previous)
    }

    /// The table the terminal shows, with each row's average against `previous`
    /// when it has the same shape ([`Self::same_shape`]): its change, or
    /// `unresolved` when the two runs' spreads overlap (`compare::delta`).
    pub fn render(&self, previous: Option<&SceneReport>) -> String {
        let comparable = previous.filter(|p| self.same_shape(p));
        let (w, h) = self.size;
        let runs = self.runs.max(1);
        let mut out = format!(
            "bench-scenes: {} — {} viewpoints x {} frames x {runs} runs at {w}x{h}, {} loads\n",
            self.scene,
            self.views.len(),
            self.frames,
            self.loads
        );
        out.push_str(&self.run_notes());
        let against = self.compared_with().0;
        out.push_str(&self.view_size_line(comparable));
        let w = self.metric_width();
        let _ = writeln!(
            out,
            "{:<w$} {:>12} {:>19} {:>12}  {:<18} vs {against} avg",
            "metric", "avg", "spread", "worst p95", "worst at"
        );
        out.push_str(&self.rows_table(comparable, against, w));
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

    /// The rows, `w` wide, against `comparable`: a quiet one only when it moved.
    fn rows_table(&self, comparable: Option<&SceneReport>, against: &str, w: usize) -> String {
        let (mut out, mut unshown) = (String::new(), 0);
        for row in &self.rows {
            let last = comparable.and_then(|p| p.row(&row.metric));
            if row.quiet && !quiet_moved(comparable.is_some(), last.map(|b| b.avg), row.avg) {
                unshown += 1;
                continue;
            }
            let delta = match last {
                Some(b) => notes::view_size_delta(b, row).unwrap_or_else(|| delta(b, row)),
                None if row.quiet => format!("absent in {against}"),
                None => String::new(),
            };
            let _ = writeln!(
                out,
                "{:<w$} {:>12} {:>19} {:>12}  {:<18} {delta}",
                row.metric,
                num(row.avg),
                spread(row),
                num(row.p95),
                row.worst
            );
        }
        let file = format!("{}.json", self.scene);
        out.push_str(&unshown_line(unshown, comparable.map(|_| against), &file));
        out
    }
}

/// The rows the run's summary shows for each scene, in order: `cpu_ms` is the
/// tick's time at the reference CPU clock, `frame_ms` as measured.
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
/// `out/bench/scenes/` holding it under its project's key (`<dir>/<key>/`); for
/// a `SCRIPT=1` run (`script`), in either one's `script/` folder first. A report
/// of the run's own kind wins; one of the other kind is returned only when the
/// baseline holds no other, and the comparison then says so.
pub fn read_baseline(dir: &Path, key: &str, scene: &str, script: bool) -> Option<SceneReport> {
    let keyed = dir.join(key);
    let folders = [dir.join("script"), keyed.join("script"), dir.into(), keyed];
    let found: Vec<SceneReport> = folders.iter().filter_map(|d| read(d, scene)).collect();
    let own = found.iter().position(|r| r.script == script);
    found.into_iter().nth(own.unwrap_or(0))
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
#[path = "quiet_tests.rs"]
mod quiet_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
