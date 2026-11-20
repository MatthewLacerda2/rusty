//! crates/dev/src/bench/ — measure before optimising (#835).
//!
//! `make bench` runs `tests/fixtures/project/scenarios/bench/bench.lua`: it loads the
//! shooter-shaped stress scene ([`stress`]) onto the default yard, warms up, then
//! `Harness.Bench(n)` steps and renders `n` frames, sampling each one, and prints
//! a short report — frame, render and GPU ms (average and p95), GPU ms per pass,
//! draw calls, triangles, dropped and culled counts, the Hi-Z culls and late set, the GPU calls
//! (passes, submits, bind groups made, buffer writes), each system's CPU ms
//! (`cpu.<system>`) and the renderer's by stage (`render.<stage>`), each set with an `other` row for
//! what none of its parts holds. Every other `Debug.Stats` counter is a row too, quiet:
//! printed only when it changed (`bench::sample`). A stage timed in parts (the Hi-Z `occlusion`
//! stage, #2401) adds `render.<stage>.<part>` and `gpu.<pass>.<part>` rows beside
//! its own, which they sit inside: no sum of a set counts them.
//!
//! The run has two windows (`window`, #2448): the quiet one above, then a
//! firefight (`bench/firefight.lua` spawning, destroying, toggling, stamping and
//! re-parenting every tick) measured as `Harness.Bench(n, "firefight")`, its rows
//! the same names prefixed `firefight.`.
//!
//! **Before and after is automatic.** The report is kept as `<out_dir>/bench.json`
//! (`out/bench/` under `make bench`) and each run prints its change against the
//! last: measuring an optimisation is run, change, run.
//!
//! **GPU rows are clock-normalised** (`clock`, #862): a fixed probe timed beside
//! every frame divides out the GPU governor's clock, so `gpu_ms` (the passes' busy
//! sum) and `gpu.<pass>` move with the work, not with how idle the GPU was;
//! `gpu.other`, untimed work inside a submit, is scaled like them.
//! `gpu_between_submits_ms`, the GPU's wait for the CPU between submits (with any
//! command at a submit's edge), is **not**: a wait does not shrink when the clock
//! rises, and scaling it would understate it most where the GPU idles furthest.
//! The three partition the frame's timed span (`rusty_render::GpuTimes`);
//! `gpu_raw_ms` is the time as measured, `gpu_clock_pct` the clock it ran at.
//!
//! **CPU rows are clock-normalised the same way** (#1719): a fixed CPU probe timed
//! right before and after each sim step scales `cpu_ms` and `cpu.<system>` to the
//! reference CPU clock; `frame_ms` is the whole tick's time as measured and
//! `cpu_clock_pct` the clock it ran at; the report says when either spreads across
//! the window, or the clock moved since the last run.
//!
//! A signal, never a gate: timings are this machine's (a CI runner's software GPU
//! makes them meaningless), only the counts portable. The scene is deterministic
//! (fixed dt, seeded scripts, no RNG in placement): one machine, the same frames.
//!
//! `make bench-scenes` ([`scenes`]) measures a game project's own scenes the same
//! way, from deterministic viewpoints, with load time and memory beside; its
//! `SCRIPT=1` adds the `script.` rows ([`verbs`]).

mod clock;
mod engine;
mod lua;
mod quiet;
mod report;
mod rig;
mod sample;
pub mod scenes;
pub mod stress;
pub mod verbs;
mod window;

pub use lua::register;
pub(crate) use lua::spec_from_lua;
pub use report::{Report, Samples, Summary};

use super::harness::Harness;
use rusty_render::ClockProbe;
use sample::{sample, Speeds};

/// Step and render `ticks` frames (fewer if the game quits), sampling each one.
/// The CPU probe runs right before and right after each sim step, the GPU probe
/// right before and right after each frame's GPU work: both outside what they scale.
pub fn measure(harness: &mut Harness, ticks: u32) -> Report {
    let probe = harness.renderer().and_then(|r| ClockProbe::new(r));
    let (mut report, samples) = measure_posed(harness, ticks, probe.as_ref(), &mut |_| {});
    report.warnings = clock::cpu_warnings(&samples);
    report
}

/// [`measure`] with the caller's GPU clock `probe` (built once, so its setup stays
/// out of every window it times) and `pose` called after each sim step and before
/// its frame is drawn: `make bench-scenes` holds its viewpoint there, whatever the
/// scripts did. Returns the report, without warnings, and the samples it summed.
pub(crate) fn measure_posed(
    harness: &mut Harness,
    ticks: u32,
    probe: Option<&ClockProbe>,
    pose: &mut dyn FnMut(&mut Harness),
) -> (Report, Samples) {
    let mut samples = Samples::default();
    // The first dispatch pays for setting the pipeline up: not a reading.
    time_probe(probe, harness);
    clock::cpu_probe_ms();
    let mut verbs = verbs::Window::start();
    for _ in 0..ticks {
        if harness.world.borrow().quit_requested() {
            break;
        }
        let cpu_before = clock::cpu_probe_ms();
        harness.step(1);
        let cpu = clock::cpu_speed(cpu_before, clock::cpu_probe_ms());
        pose(harness);
        let before = time_probe(probe, harness);
        let frame = harness.render_frame();
        let after = time_probe(probe, harness);
        let gpu = clock::speed(before, after);
        let speed = Speeds { cpu, gpu };
        sample(&mut samples, &harness.stats.borrow(), frame.as_ref(), speed);
        verbs.sample(&mut samples, &harness.stats.borrow(), speed.cpu);
        samples.end_frame();
    }
    (samples.report(), samples)
}

fn time_probe(probe: Option<&ClockProbe>, harness: &mut Harness) -> Option<f64> {
    let probe = probe?;
    let renderer = harness.renderer()?;
    probe.time(renderer)
}

#[cfg(test)]
#[path = "firefight_shot_tests.rs"]
mod firefight_shot_tests;
#[cfg(test)]
#[path = "firefight_tests.rs"]
mod firefight_tests;
#[cfg(test)]
#[path = "move_cost_tests.rs"]
mod move_cost_tests;
#[cfg(test)]
#[path = "query_cost_tests.rs"]
mod query_cost_tests;
#[cfg(test)]
#[path = "run_tests.rs"]
mod run_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
