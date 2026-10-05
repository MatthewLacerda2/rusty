//! src/dev/bench/ — measure before optimising (#835).
//!
//! `make bench` runs `project/scenarios/bench/bench.lua`: it loads the
//! shooter-shaped stress scene ([`stress`]) onto the default yard, warms up, then
//! `Harness.Bench(n)` steps and renders `n` frames, sampling each one, and prints
//! a short report — frame, render and GPU ms (average and p95), GPU ms per pass,
//! draw calls, triangles, dropped and culled counts, and the heavy systems.
//!
//! **Before and after is automatic.** The report is kept as `<out_dir>/bench.json`
//! (`out/bench/` under `make bench`) and each run prints its change against the one
//! before, so measuring an optimisation is: run, change, run.
//!
//! It is an informational signal, never a gate: timings are this machine's, and a
//! CI runner's software GPU makes them meaningless. Only the counts are portable.
//! The scene is deterministic (fixed dt, seeded scripts, no RNG in placement), so
//! two runs on one machine measure the same frames.

mod lua;
mod report;
mod rig;
pub mod stress;

use std::path::Path;

pub use lua::register;
pub use report::{Report, Samples, Summary};

use super::capture::RenderedFrame;
use super::harness::Harness;
use crate::core::frame_stats::FrameStats;
use crate::render::GpuPass;

/// The render counters a bench row reports, in order.
const COUNTERS: [&str; 5] = [
    "draw_calls",
    "triangles",
    "lights_dropped",
    "culled_entities",
    "lights_culled",
];
/// The systems a bench row reports, in order: the sim's heavy hitters.
const SYSTEMS: [&str; 4] = ["update_scripts", "tick_nav", "step_physics", "animate"];

/// Step and render `ticks` frames (fewer if the game quits), sampling each one.
pub fn measure(harness: &mut Harness, ticks: u32) -> Report {
    let mut samples = Samples::default();
    for _ in 0..ticks {
        if harness.world.borrow().quit_requested() {
            break;
        }
        harness.step(1);
        let frame = harness.render_frame();
        sample(&mut samples, &harness.stats.borrow(), frame.as_ref());
        samples.end_frame();
    }
    samples.report()
}

/// One frame's row values: the sim's CPU time, what rendering it cost, its
/// counters and the heavy systems' times. Every GPU pass is sampled every frame
/// (zero when it did not run), so the rows are the same from run to run.
fn sample(samples: &mut Samples, stats: &FrameStats, frame: Option<&RenderedFrame>) {
    if let Some(s) = stats.get("frame_ms") {
        samples.push("frame_ms", s.last);
    }
    if let Some(frame) = frame {
        samples.push("renderer_ms", frame.cpu_ms);
        if let Some(gpu) = &frame.gpu {
            samples.push("gpu_ms", gpu.total_ms());
            for pass in GpuPass::ALL {
                let ms = gpu.passes.iter().find(|(p, _)| *p == pass);
                samples.push(&format!("gpu.{}", pass.name()), ms.map_or(0.0, |m| m.1));
            }
        }
        for (key, value) in frame.counters.pairs() {
            if COUNTERS.contains(&key) {
                samples.push(key, value as f64);
            }
        }
    }
    for system in SYSTEMS {
        if let Some(s) = stats.systems.get(system) {
            samples.push(system, s.last);
        }
    }
}

/// Measure `ticks` frames, print the report with its change against the last one
/// kept in `out_dir`, and keep this one there instead. Returns the report.
pub fn run(harness: &mut Harness, ticks: u32) -> Report {
    let report = measure(harness, ticks);
    let out_dir = harness.out_dir().to_path_buf();
    let previous = read(&out_dir);
    let mut text = report.render(previous.as_ref());
    if !report.metrics.iter().any(|(k, _)| k == "renderer_ms") {
        text.push_str("(no GPU or software adapter: nothing was rendered)\n");
    } else if !report.metrics.iter().any(|(k, _)| k == "gpu_ms") {
        text.push_str("(this adapter has no timestamp queries: no GPU times)\n");
    }
    println!("{text}");
    if let Err(e) = write(&out_dir, &report, &text) {
        harness.console.borrow_mut().error(format!("[Bench] {e}"));
    }
    harness.log(format!("bench: {} frames measured", report.frames));
    report
}

/// The last run's report, if `out_dir` holds a readable one.
fn read(out_dir: &Path) -> Option<Report> {
    let json = std::fs::read_to_string(out_dir.join("bench.json")).ok()?;
    serde_json::from_str(&json).ok()
}

fn write(out_dir: &Path, report: &Report, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let json = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    std::fs::write(out_dir.join("bench.json"), json)?;
    std::fs::write(out_dir.join("bench.txt"), text)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
