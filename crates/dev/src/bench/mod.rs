//! crates/dev/src/bench/ — measure before optimising (#835).
//!
//! `make bench` runs `tests/fixtures/project/scenarios/bench/bench.lua`: it loads the
//! shooter-shaped stress scene ([`stress`]) onto the default yard, warms up, then
//! `Harness.Bench(n)` steps and renders `n` frames, sampling each one, and prints
//! a short report — frame, render, wall and GPU ms (average and p95), GPU ms per pass,
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
//! **`wall_ms` is the frame end to end** (#2494): its sim step and its draw on the
//! wall, from the step's start to the GPU going idle, so it is the one row that
//! moves when a change shifts *when* the GPU starts or how long the CPU waits on
//! it (`frame_ms`, `renderer_ms` and `gpu_ms` can stay level while it grows). It
//! leaves out what `frame_ms` leaves out of the tick (`loads_ms`, `taps_ms`,
//! `probe_ms`), the bench's own readbacks after the GPU finished (the settled
//! exposure, the timestamps, the stats' recording), the clock probes and the
//! bench's pose, and is sampled only for a frame that was drawn. A raw time: it
//! moves with both clocks (`cpu_clock_pct`, `gpu_clock_pct`), and the clock
//! warnings name it beside `frame_ms`. It is not the windowed game's frame time:
//! the bench draws one frame after another offscreen, each finished before the
//! next steps.
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

use super::capture::RenderedFrame;
use super::harness::Harness;
use super::stats;
use rusty_core::frame_stats::FrameStats;
use rusty_render::ClockProbe;
use sample::{sample, Speeds};

/// Step and render `ticks` frames (fewer if the game quits), sampling each one,
/// `wall_ms` among the rows (the module docs). The CPU probe runs right before
/// and right after each sim step, the GPU probe right before and right after each
/// frame's GPU work: both outside what they scale.
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
        let (frame, speed) = step_and_draw(harness, probe, pose);
        let drawn = frame.as_ref().map(|(f, wall_ms)| (f, *wall_ms));
        sample(&mut samples, &harness.stats.borrow(), drawn, speed);
        verbs.sample(&mut samples, &harness.stats.borrow(), speed.cpu);
        samples.end_frame();
    }
    (samples.report(), samples)
}

/// What `frame_ms` leaves out of the tick: its parts and the probe's own work.
const BESIDE: [&str; 3] = ["loads_ms", "taps_ms", "probe_ms"];

/// Step one frame, `pose`, draw it: the frame with its `wall_ms`, when one was
/// drawn, and its clock speeds. `wall_ms` is the step less what `frame_ms` leaves
/// out of the tick ([`BESIDE`]), plus the draw until the GPU finished, before the
/// readbacks; the probes and the pose are out.
fn step_and_draw(
    harness: &mut Harness,
    probe: Option<&ClockProbe>,
    pose: &mut dyn FnMut(&mut Harness),
) -> (Option<(RenderedFrame, f64)>, Speeds) {
    let cpu_before = clock::cpu_probe_ms();
    let recorded = beside_counts(&harness.stats.borrow());
    let tick = stats::now();
    harness.step(1);
    let step_ms = stats::ms_since(tick) - beside_ms(&harness.stats.borrow(), recorded);
    let cpu = clock::cpu_speed(cpu_before, clock::cpu_probe_ms());
    pose(harness);
    let before = time_probe(probe, harness);
    let start = stats::now();
    let frame = harness.render_frame();
    let idle = harness.idle_at().unwrap_or(start);
    let gpu = clock::speed(before, time_probe(probe, harness));
    let wall_ms = step_ms.max(0.0) + stats::ms_between(start, idle);
    (frame.map(|f| (f, wall_ms)), Speeds { cpu, gpu })
}

/// The stats' frame count and each [`BESIDE`] row's sample count, before a tick.
fn beside_counts(stats: &FrameStats) -> (u64, [u64; 3]) {
    let samples = BESIDE.map(|k| stats.get(k).map_or(0, |s| s.samples));
    (stats.frames, samples)
}

/// The [`BESIDE`] rows the tick just closed recorded, summed: a row counts when
/// its sample count moved since `before`, or every row present when the tick
/// restarted the stats (a Play session's first frame).
fn beside_ms(stats: &FrameStats, (frames, before): (u64, [u64; 3])) -> f64 {
    let restarted = stats.frames <= frames;
    let rows = BESIDE.iter().zip(before);
    let ran = rows.filter_map(|(k, n)| stats.get(k).filter(|s| restarted || s.samples != n));
    ran.map(|s| s.last).sum()
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
#[cfg(test)]
#[path = "wall_tests.rs"]
mod wall_tests;
