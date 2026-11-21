//! A bench run's measured windows (#2448). `Harness.Bench(n)` starts a run: it
//! measures its first window against the last run kept in the out dir.
//! `Harness.Bench(n, "firefight")` measures a later window of the **same** run,
//! after whatever the scenario changed in between: its rows are kept under the
//! window's prefix (`firefight.cpu_ms`, `firefight.cpu.<system>`, …) beside the
//! first window's, so one `bench.json` holds the run and every row compares
//! against its own row of the last run. A window prints as its own table, its rows
//! by their plain names, so each window's sums (`cpu_ms` = its `cpu.` rows) read
//! as the first window's do.
//!
//! **The kept report always holds the last complete measurement of each window.**
//! It is written after every window, and a window this run has not measured (yet,
//! or at all: a run stopped early) keeps the last run's rows for it, so a stopped
//! run never erases a window it never reached.

use super::report::{read, tick_timed_since, write, Report};
use super::{clock, measure};
use crate::harness::Harness;
use rusty_core::project::EngineStamp;

/// The run so far: the last run it compares against, read once at its start,
/// and what it has measured.
#[derive(Default)]
pub struct Run {
    /// The kept report of the run before this one (`None` on a fresh out dir).
    previous: Option<Report>,
    /// Every window measured so far, the later ones' rows prefixed.
    kept: Report,
    /// Each window's printed table, in order: `bench.txt`.
    text: String,
    started: bool,
    /// Whether this run measured its first (unprefixed) window.
    first_measured: bool,
    /// The engine that measured the run, stamped once: its mismatch warning is
    /// the run's, printed under the first window it measures.
    engine: Option<EngineStamp>,
}

impl Run {
    /// Start a run: measure `ticks` frames as its first window, print the table
    /// with its change against the last run kept in the out dir, and keep the run
    /// there instead. Returns the window's report.
    pub fn first(&mut self, harness: &mut Harness, ticks: u32) -> Report {
        *self = Run {
            previous: read(harness.out_dir()),
            started: true,
            ..Run::default()
        };
        self.window(harness, ticks, None)
    }

    /// Measure `ticks` more frames as the run's window `name`, its rows prefixed
    /// `<name>.` in the returned report and in the kept one. Starts the run when
    /// none has (a scenario may measure only a later window). An error, measuring
    /// nothing, for a name that is not a lowercase word or was measured already.
    pub fn then(
        &mut self,
        harness: &mut Harness,
        ticks: u32,
        name: &str,
    ) -> Result<Report, String> {
        let word = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_';
        if name.is_empty() || !name.chars().all(word) {
            return Err(format!(
                "Bench: a window's name is a lowercase word (a-z, 0-9, _), got `{name}`"
            ));
        }
        if self.kept.windows.iter().any(|w| w == name) {
            return Err(format!(
                "Bench: window `{name}` was already measured this run"
            ));
        }
        if !self.started {
            self.previous = read(harness.out_dir());
            self.started = true;
        }
        Ok(self.window(harness, ticks, Some(name)))
    }

    fn window(&mut self, harness: &mut Harness, ticks: u32, name: Option<&str>) -> Report {
        let mut report = measure(harness, ticks);
        if self.engine.is_none() {
            let engine = rusty_core::project::engine_content::stamp();
            report.warnings.extend(engine.warning());
            self.engine = Some(engine);
        }
        report.engine.clone_from(&self.engine);
        let last = self.previous.as_ref().map(|p| match name {
            Some(name) => window_of(p, name),
            None => p.clone(),
        });
        let kept = last.as_ref();
        let untimed = kept.and_then(|k| tick_timed_since(|m| report.has(m), |m| k.has(m)));
        let drift = clock::cpu_drift(&report, kept);
        report.warnings.extend(drift.into_iter().chain(untimed));
        let mut text = match name {
            Some(name) => format!("window `{name}` (its rows kept as `{name}.<row>`)\n"),
            None => String::new(),
        };
        text.push_str(&report.render(kept));
        if name.is_none() {
            text.push_str(&footnotes(&report));
        }
        println!("{text}");
        self.text.push_str(&text);
        let report = match name {
            Some(name) => prefixed(report, name),
            None => report,
        };
        self.keep(&report, name);
        let out_dir = harness.out_dir().to_path_buf();
        if let Err(e) = write(&out_dir, &self.with_unmeasured(), &self.text) {
            harness.console.borrow_mut().error(format!("[Bench] {e}"));
        }
        let what = name.map_or(String::new(), |n| format!(" in window `{n}`"));
        harness.log(format!("bench: {} frames measured{what}", report.frames));
        report
    }

    /// Add a window's (prefixed) `report` to the run's: the first window gives the
    /// run its frame count and engine; a later one adds its rows and warnings.
    fn keep(&mut self, report: &Report, name: Option<&str>) {
        let Some(name) = name else {
            self.kept = report.clone();
            self.first_measured = true;
            return;
        };
        self.kept.windows.push(name.to_string());
        self.kept.metrics.extend(report.metrics.iter().cloned());
        self.kept.warnings.extend(report.warnings.iter().cloned());
        if self.kept.engine.is_none() {
            self.kept.frames = report.frames;
            self.kept.engine.clone_from(&report.engine);
        }
    }

    /// What is kept: this run's windows, and for each window the last run
    /// measured and this one has not, the last run's rows of it.
    fn with_unmeasured(&self) -> Report {
        let mut out = self.kept.clone();
        let Some(last) = &self.previous else {
            return out;
        };
        let of = |w: &str, m: &str| m.strip_prefix(w).is_some_and(|r| r.starts_with('.'));
        let windowed = |m: &str| last.windows.iter().any(|w| of(w, m));
        if !self.first_measured {
            let plain = last.metrics.iter().filter(|(m, _)| !windowed(m));
            let mut metrics: Vec<_> = plain.cloned().collect();
            metrics.append(&mut out.metrics);
            out.metrics = metrics;
            out.frames = last.frames;
            out.engine.clone_from(&last.engine);
        }
        for w in &last.windows {
            if out.windows.contains(w) {
                continue;
            }
            let rows = last.metrics.iter().filter(|(m, _)| of(w, m));
            out.metrics.extend(rows.cloned());
            out.windows.push(w.clone());
        }
        out
    }
}

/// `report` with every row renamed `<name>.<row>`, and every warning said to be
/// the window's.
pub fn prefixed(mut report: Report, name: &str) -> Report {
    for (metric, _) in &mut report.metrics {
        *metric = format!("{name}.{metric}");
    }
    for warning in &mut report.warnings {
        *warning = format!("window `{name}`: {warning}");
    }
    report
}

/// Window `name`'s rows of a kept run, by their plain names: what that window
/// compares against (no rows when the kept run never measured it, so every row
/// reads "absent on the last run").
pub fn window_of(run: &Report, name: &str) -> Report {
    let prefix = format!("{name}.");
    let metrics = run.metrics.iter().filter_map(|(metric, s)| {
        let plain = metric.strip_prefix(&prefix)?;
        Some((plain.to_string(), *s))
    });
    Report {
        frames: run.frames,
        metrics: metrics.collect(),
        warnings: Vec::new(),
        engine: run.engine.clone(),
        windows: Vec::new(),
    }
}

/// What the first window's GPU and CPU rows are, under its table.
fn footnotes(report: &Report) -> String {
    let mut text = String::new();
    if !report.has("renderer_ms") {
        text.push_str("(no GPU or software adapter: nothing was rendered)\n");
    } else if !report.has("gpu_ms") {
        text.push_str("(this adapter has no timestamp queries: no GPU times)\n");
    } else {
        text.push_str(
            "(gpu_ms, gpu.<pass>, gpu.other: ms at the reference clock, gpu_ms the passes' \
             busy sum and gpu.other untimed work inside a submit; gpu_between_submits_ms the \
             GPU waiting on the CPU, as measured; gpu_raw_ms as measured, at gpu_clock_pct \
             of the reference)\n",
        );
    }
    text.push_str(
        "(cpu_ms, cpu.<system>: ms at the reference CPU clock; frame_ms as measured, \
         at cpu_clock_pct of it)\n",
    );
    if report.has("wall_ms") {
        text.push_str(
            "(wall_ms: the frame end to end on the wall, its sim step and its draw until the \
             GPU finished, as measured: it moves with cpu_clock_pct and gpu_clock_pct)\n",
        );
    }
    text
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
