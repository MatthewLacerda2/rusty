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

pub const PROFILING_NOTE: &str = "(timed on the profiling build: release with frame pointers \
     forced on, for PROFILE=1; within the release build's spread where measured)";

pub const SCRIPT_NOTE: &str = "script verbs timed (SCRIPT=1): script.<verb> is a verb's own ms, \
     script.lua what of cpu.update_scripts neither a verb nor the timers (script.overhead) hold; \
     the timers cost time, so \
     cpu.update_scripts compares only with another SCRIPT=1 run\n";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub metric: String,
    pub avg: f64,
    pub p95: f64,
    pub worst: String,
    pub lo: f64,
    pub hi: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub quiet: bool,
}

impl Row {
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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SceneReport {
    pub scene: String,
    pub views: Vec<ViewKey>,
    pub frames: u32,
    pub runs: u32,
    #[serde(default)]
    pub loads: u32,
    pub measured: u32,
    pub baseline: Option<String>,
    pub size: (u32, u32),
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub script: bool,
    pub rows: Vec<Row>,
    pub gpu_mb_source: Option<String>,
    pub warnings: Vec<String>,
    pub engine: Option<EngineStamp>,
}

impl SceneReport {
    pub fn row(&self, metric: &str) -> Option<&Row> {
        self.rows.iter().find(|r| r.metric == metric)
    }

    pub fn push_value(&mut self, metric: &str, value: f64) {
        self.rows.push(Row::value(metric.into(), value));
    }

    fn metric_width(&self) -> usize {
        self.rows
            .iter()
            .map(|r| r.metric.len())
            .fold(30, usize::max)
    }

    pub fn same_shape(&self, previous: &SceneReport) -> bool {
        let shape = |r: &SceneReport| (r.size, r.views.clone(), r.frames, r.script);
        shape(self) == shape(previous)
    }

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

const SUMMARY: [&str; 7] = [
    "cpu_ms",
    "frame_ms",
    "wall_ms",
    "gpu_ms",
    "load_ms",
    "rss_peak_mb",
    "gpu_mb",
];

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

pub fn path(out_dir: &Path, scene: &str) -> PathBuf {
    out_dir.join(format!("{scene}.json"))
}

pub fn read(out_dir: &Path, scene: &str) -> Option<SceneReport> {
    let json = std::fs::read_to_string(path(out_dir, scene)).ok()?;
    serde_json::from_str(&json).ok()
}

pub fn read_baseline(dir: &Path, key: &str, scene: &str, script: bool) -> Option<SceneReport> {
    let keyed = dir.join(key);
    let folders = [dir.join("script"), keyed.join("script"), dir.into(), keyed];
    let found: Vec<SceneReport> = folders.iter().filter_map(|d| read(d, scene)).collect();
    let own = found.iter().position(|r| r.script == script);
    found.into_iter().nth(own.unwrap_or(0))
}

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
#[cfg(test)]
#[path = "wall_tests.rs"]
mod wall_tests;
