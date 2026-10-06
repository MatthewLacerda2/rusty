//! src/dev/bench/ — measure before optimising (#835).
//!
//! `make bench` runs `tests/fixtures/project/scenarios/bench/bench.lua`: it loads the
//! shooter-shaped stress scene ([`stress`]) onto the default yard, warms up, then
//! `Harness.Bench(n)` steps and renders `n` frames, sampling each one, and prints
//! a short report — frame, render and GPU ms (average and p95), GPU ms per pass,
//! draw calls, triangles, dropped and culled counts, and the heavy systems.
//!
//! **Before and after is automatic.** The report is kept as `<out_dir>/bench.json`
//! (`out/bench/` under `make bench`) and each run prints its change against the one
//! before, so measuring an optimisation is: run, change, run.
//!
//! **GPU rows are clock-normalised** (`clock`, #862): a fixed probe timed beside
//! every frame divides out the GPU governor's clock, so `gpu_ms` and `gpu.<pass>`
//! move with the work, not with how idle the GPU was. `gpu_raw_ms` is the time as
//! measured and `gpu_clock_pct` the clock it ran at.
//!
//! It is an informational signal, never a gate: timings are this machine's, and a
//! CI runner's software GPU makes them meaningless. Only the counts are portable.
//! The scene is deterministic (fixed dt, seeded scripts, no RNG in placement), so
//! two runs on one machine measure the same frames.

mod clock;
mod lua;
mod report;
mod rig;
pub mod stress;

use std::path::Path;

pub use lua::register;
pub use report::{Report, Samples, Summary};

use self::clock::ClockProbe;
use super::capture::RenderedFrame;
use super::harness::Harness;
use crate::core::frame_stats::FrameStats;
use crate::render::GpuPass;

/// The render counters a bench row reports, in order.
const COUNTERS: [&str; 6] = [
    "draw_calls",
    "triangles",
    "lights_dropped",
    "cluster_lights_dropped",
    "culled_entities",
    "lights_culled",
];
/// The systems a bench row reports, in order: the sim's heavy hitters.
const SYSTEMS: [&str; 4] = ["update_scripts", "tick_nav", "step_physics", "animate"];

/// Step and render `ticks` frames (fewer if the game quits), sampling each one.
/// The clock probe runs right before and right after each frame's GPU work.
pub fn measure(harness: &mut Harness, ticks: u32) -> Report {
    let mut samples = Samples::default();
    let probe = harness
        .renderer()
        .and_then(|r| ClockProbe::new(&r.device, &r.queue));
    // The first dispatch pays for setting the pipeline up: not a reading.
    time_probe(probe.as_ref(), harness);
    for _ in 0..ticks {
        if harness.world.borrow().quit_requested() {
            break;
        }
        harness.step(1);
        let before = time_probe(probe.as_ref(), harness);
        let frame = harness.render_frame();
        let after = time_probe(probe.as_ref(), harness);
        let speed = clock::speed(before, after);
        sample(&mut samples, &harness.stats.borrow(), frame.as_ref(), speed);
        samples.end_frame();
    }
    samples.report()
}

fn time_probe(probe: Option<&ClockProbe>, harness: &mut Harness) -> Option<f64> {
    let probe = probe?;
    let renderer = harness.renderer()?;
    probe.time(&renderer.device, &renderer.queue)
}

/// One frame's row values: the sim's CPU time, what rendering it cost, its
/// counters and the heavy systems' times. Every GPU pass is sampled every frame
/// (zero when it did not run), so the rows are the same from run to run. GPU
/// times are scaled by the frame's clock `speed`; a frame the probe missed has
/// no GPU rows.
fn sample(
    samples: &mut Samples,
    stats: &FrameStats,
    frame: Option<&RenderedFrame>,
    speed: Option<f64>,
) {
    if let Some(s) = stats.get("frame_ms") {
        samples.push("frame_ms", s.last);
    }
    if let Some(frame) = frame {
        samples.push("renderer_ms", frame.cpu_ms);
        if let (Some(gpu), Some(speed)) = (&frame.gpu, speed) {
            samples.push("gpu_ms", gpu.total_ms() * speed);
            for pass in GpuPass::ALL {
                let ms = gpu.passes.iter().find(|(p, _)| *p == pass);
                let ms = ms.map_or(0.0, |m| m.1 * speed);
                samples.push(&format!("gpu.{}", pass.name()), ms);
            }
            samples.push("gpu_raw_ms", gpu.total_ms());
            samples.push("gpu_clock_pct", speed * 100.0);
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
    } else {
        text.push_str(
            "(gpu_ms, gpu.<pass>: ms at the reference clock; gpu_raw_ms as measured, \
             at gpu_clock_pct of it)\n",
        );
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

#[cfg(test)]
#[path = "run_tests.rs"]
mod run_tests;
