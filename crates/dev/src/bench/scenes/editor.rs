//! The editor round's rows (#2314): what an editor frame costs and what Play,
//! Stop and a save cost, on the scene the game rounds measured.
//!
//! `rusty-shell` drives the round (`shell::editor::capture::bench`: the editor it
//! draws sits above this crate) and hands every number to a [`Meter`], which
//! turns them into rows the scene's report keeps beside the game rows:
//!
//! - `editor.edit_ms`: the world's edit-mode frame (`GameWorld::edit_frame`: held
//!   conversions, the UI's texture sizes, model writes followed, skin palettes),
//!   which the windowed editor runs every frame outside Play;
//! - `editor.ui_ms`: `EditorUi::draw` and egui's tessellation; both it and
//!   `editor.edit_ms` at the reference CPU clock, as `cpu_ms` is: the CPU probe is
//!   timed right before and after them ([`UiTimer`]), and `editor.cpu_clock_pct`
//!   is the clock they ran at;
//! - `editor.renderer_ms`, `editor.render.<stage>`, `editor.gpu.<pass>` and the
//!   counters: the Scene view's frame, sampled as a viewpoint's is
//!   (`bench::sample`), under the `editor.` prefix so they never mix with the game
//!   rows of the same name;
//! - `play_enter_ms`, `play_stop_ms` and `save_ms`: one sample a run
//!   ([`Meter::single`]), wall clock like `load_ms`, each with its spread over the
//!   runs; `play_rss_mb`: the first run's alone ([`Meter::first_run`]), since the
//!   pages a Stop frees stay resident and the later runs' Play re-uses them.
//!
//! A run's frames are one window: a CPU clock that moves across it, or a `ui_ms`
//! that spreads, warns as a viewpoint's does (`fold::spread_warnings`).
//!
//! The round runs in a child process of its own (`bench-scenes --editor-out`),
//! which writes an [`EditorRound`]; the scene's timed child reads it (`--editor`)
//! into its report, so the game rows' `rss_peak_mb` never holds a Play snapshot.
//!
//! stored-format: the round a child hands the scene's bench, deleted after

use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::super::clock::{self, ClockRows, Spread};
use super::super::{sample::sample_render, Samples};
use super::fold;
use super::report::Row;
use crate::capture::RenderedFrame;

/// The prefix of every frame row the round measures.
pub const PREFIX: &str = "editor.";

/// What opens every warning about the round, so it never reads as the game rows'.
pub const SAID: &str = "editor round: ";

/// The rows the round's clock warnings name: `editor.ui_ms` at the reference
/// clock, the single samples on the wall clock.
pub const CLOCK_ROWS: ClockRows = ClockRows {
    reference: "editor.ui_ms",
    wall: &["play_enter_ms", "play_stop_ms", "save_ms"],
};

/// The round's rows and warnings, as the scene's report takes them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditorRound {
    pub rows: Vec<Row>,
    pub warnings: Vec<String>,
}

/// Keep `round` at `file`, for the scene's bench to fold (`--editor-out`).
pub fn write(file: &Path, round: &EditorRound) -> Result<(), String> {
    let json = serde_json::to_string(round).map_err(|e| e.to_string())?;
    // lint: allow(asset-write) a round handed to the scene's bench process, deleted after
    std::fs::write(file, json).map_err(|e| format!("{}: {e}", file.display()))
}

/// The round kept at `file` (`--editor`).
pub fn read(file: &Path) -> Result<EditorRound, String> {
    let json = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    serde_json::from_str(&json).map_err(|e| format!("{}: {e}", file.display()))
}

/// Times one frame's CPU work, the edit frame then the UI, the CPU probe before
/// and after both.
pub struct UiTimer {
    probe: f64,
    start: Instant,
    edit_ms: f64,
}

/// What [`UiTimer`] measured: the edit frame's ms, the UI's, and the CPU's speed
/// against the reference clock (`None` where the probe read nothing).
#[derive(Clone, Copy, Debug)]
pub struct UiTime {
    pub edit_ms: f64,
    pub ms: f64,
    pub speed: Option<f64>,
}

impl UiTimer {
    /// Time the probe, then start the clock on the edit frame.
    pub fn start() -> Self {
        let probe = clock::cpu_probe_ms();
        let start = Instant::now();
        Self {
            probe,
            start,
            edit_ms: 0.0,
        }
    }

    /// The edit frame is done: keep its time and start the clock on the UI.
    pub fn edit_done(&mut self) {
        self.edit_ms = ms_since(self.start);
        self.start = Instant::now();
    }

    /// Stop the clock on the UI, then time the probe again.
    pub fn stop(self) -> UiTime {
        let ms = ms_since(self.start);
        let speed = clock::cpu_speed(self.probe, clock::cpu_probe_ms());
        UiTime {
            edit_ms: self.edit_ms,
            ms,
            speed,
        }
    }
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// The GPU's speed against the reference clock from the probe timed before
/// and after a frame's GPU work (`rusty_render::ClockProbe::time`), as a
/// viewpoint's frame reads it.
pub fn gpu_speed(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    clock::speed(before, after)
}

/// Collects the round's numbers, run by run, into an [`EditorRound`].
#[derive(Default)]
pub struct Meter {
    samples: Samples,
    singles: Vec<Row>,
    runs: Vec<Vec<Row>>,
    spreads: Vec<(String, Vec<(Spread, f64)>)>,
}

impl Meter {
    pub fn new() -> Self {
        Self::default()
    }

    /// One measured frame: its UI time, and the viewport frame when one rendered,
    /// with the GPU's speed over it.
    pub fn frame(&mut self, ui: UiTime, rendered: Option<&RenderedFrame>, gpu: Option<f64>) {
        if let Some(cpu) = ui.speed {
            self.samples.push("edit_ms", ui.edit_ms * cpu);
            self.samples.push("ui_ms", ui.ms * cpu);
            self.samples.push("cpu_clock_pct", cpu * 100.0);
        }
        if let Some(frame) = rendered {
            sample_render(&mut self.samples, frame, gpu);
        }
        self.samples.end_frame();
    }

    /// This run's one sample of `metric` (`play_enter_ms`, ...).
    pub fn single(&mut self, metric: &str, value: f64) {
        self.singles.push(Row::value(metric.into(), value));
    }

    /// Whether the run being measured is the first: what only a fresh process
    /// measures (`play_rss_mb`) is sampled then alone.
    pub fn first_run(&self) -> bool {
        self.runs.is_empty()
    }

    /// End a run: its frames folded into prefixed rows, its singles after them.
    pub fn end_run(&mut self) {
        let samples = std::mem::take(&mut self.samples);
        let n = self.runs.len() + 1;
        let window = clock::spreads_of(samples.values("cpu_clock_pct"), samples.values("ui_ms"));
        self.spreads.push((format!("editor run {n}"), window));
        let mut rows = fold::aggregate(&[("editor".into(), samples.report())]);
        for row in &mut rows {
            row.metric.insert_str(0, PREFIX);
        }
        rows.append(&mut self.singles);
        self.runs.push(rows);
    }

    /// The rows over every run, each with its spread, and the windows' warnings.
    pub fn finish(self) -> EditorRound {
        let warnings = fold::spread_warnings(&self.spreads, CLOCK_ROWS);
        EditorRound {
            rows: fold::over_runs(&self.runs),
            warnings: warnings.iter().map(|w| format!("{SAID}{w}")).collect(),
        }
    }
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "editor_report_tests.rs"]
mod report_tests;
