//! stored-format: the round a child hands the scene's bench, deleted after
//! The editor round's rows: `rusty-shell` drives the round (`shell::editor::capture::bench`,
//! above this crate) and hands each number to a [`Meter`].

use std::path::Path;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::super::clock::{self, ClockRows, Spread};
use super::super::{sample::sample_render, Samples};
use super::fold;
use super::report::Row;
use crate::capture::RenderedFrame;

pub const PREFIX: &str = "editor.";

pub const SAID: &str = "editor round: ";

pub const CLOCK_ROWS: ClockRows = ClockRows {
    reference: "editor.ui_ms",
    wall: &["editor.wall_ms", "play_enter_ms", "play_stop_ms", "save_ms"],
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditorRound {
    pub rows: Vec<Row>,
    pub warnings: Vec<String>,
}

pub fn write(file: &Path, round: &EditorRound) -> Result<(), String> {
    let json = serde_json::to_string(round).map_err(|e| e.to_string())?;
    // lint: allow(asset-write) a round handed to the scene's bench process, deleted after
    std::fs::write(file, json).map_err(|e| format!("{}: {e}", file.display()))
}

pub fn read(file: &Path) -> Result<EditorRound, String> {
    let json = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    serde_json::from_str(&json).map_err(|e| format!("{}: {e}", file.display()))
}

pub struct UiTimer {
    probe: f64,
    start: Instant,
    edit_ms: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct UiTime {
    pub edit_ms: f64,
    pub ms: f64,
    pub speed: Option<f64>,
}

impl UiTimer {
    pub fn start() -> Self {
        let probe = clock::cpu_probe_ms();
        let start = Instant::now();
        Self {
            probe,
            start,
            edit_ms: 0.0,
        }
    }

    pub fn edit_done(&mut self) {
        self.edit_ms = ms_since(self.start);
        self.start = Instant::now();
    }

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

pub fn gpu_speed(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    clock::speed(before, after)
}

#[derive(Clone, Debug)]
pub struct Drawn {
    pub frame: RenderedFrame,
    pub view: (u32, u32),
}

pub const VIEW_ROWS: [&str; 2] = ["editor.view_w", "editor.view_h"];

#[derive(Default)]
pub struct Meter {
    samples: Samples,
    singles: Vec<Row>,
    runs: Vec<Vec<Row>>,
    spreads: Vec<(String, Vec<(Spread, f64)>)>,
    view: Option<(u32, u32)>,
}

impl Meter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn frame(&mut self, ui: UiTime, drawn: Option<&Drawn>, gpu: Option<f64>, wall_ms: f64) {
        if let Some(cpu) = ui.speed {
            self.samples.push("edit_ms", ui.edit_ms * cpu);
            self.samples.push("ui_ms", ui.ms * cpu);
            self.samples.push("cpu_clock_pct", cpu * 100.0);
        }
        if let Some(drawn) = drawn {
            sample_render(&mut self.samples, &drawn.frame, wall_ms, gpu);
            self.view = Some(drawn.view);
        }
        self.samples.end_frame();
    }

    pub fn single(&mut self, metric: &str, value: f64) {
        self.singles.push(Row::value(metric.into(), value));
    }

    pub fn first_run(&self) -> bool {
        self.runs.is_empty()
    }

    pub fn end_run(&mut self) {
        let samples = std::mem::take(&mut self.samples);
        let n = self.runs.len() + 1;
        let window = clock::spreads_of(samples.values("cpu_clock_pct"), samples.values("ui_ms"));
        self.spreads.push((format!("editor run {n}"), window));
        let mut rows = fold::aggregate(&[("editor".into(), samples.report())]);
        for row in &mut rows {
            row.metric.insert_str(0, PREFIX);
        }
        if let Some((w, h)) = self.view.take() {
            for (metric, px) in VIEW_ROWS.into_iter().zip([w, h]) {
                rows.push(Row::value(metric.into(), f64::from(px)));
            }
        }
        rows.append(&mut self.singles);
        self.runs.push(rows);
    }

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
