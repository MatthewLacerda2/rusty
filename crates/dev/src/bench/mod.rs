//! crates/dev/src/bench/ — measure before optimising (#835).
//!
//! `make bench` runs `tests/fixtures/project/scenarios/bench/bench.lua`: it loads the
//! shooter-shaped stress scene ([`stress`]) onto the default yard, warms up, then
//! `Harness.Bench(n)` steps and renders `n` frames, sampling each one, and prints
//! a short report — frame, render and GPU ms (average and p95), GPU ms per pass,
//! draw calls, triangles, dropped and culled counts, the Hi-Z culls and late set, the GPU calls
//! (passes, submits, bind groups made, buffer writes), each system's CPU ms
//! (`cpu.<system>`) and the renderer's by stage (`render.<stage>`), each set with an `other` row for
//! what none of its parts holds. A stage timed in parts (the Hi-Z `occlusion`
//! stage, #2401) adds `render.<stage>.<part>` and `gpu.<pass>.<part>` rows beside
//! its own, which they sit inside: no sum of a set counts them.
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
//! reference CPU clock; `frame_ms` is the sim's time as measured and
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
mod lua;
mod report;
mod rig;
pub mod scenes;
pub mod stress;
pub mod verbs;

pub use lua::register;
pub(crate) use lua::spec_from_lua;
pub use report::{Report, Samples, Summary};

use super::capture::RenderedFrame;
use super::harness::Harness;
use report::{read, write};
use rusty_core::frame_stats::FrameStats;
use rusty_render::{ClockProbe, GpuPart, GpuPass};

/// The render counters a bench row reports, in order.
const COUNTERS: [&str; 18] = [
    "draw_calls",
    "triangles",
    "lights_dropped",
    "cluster_lights_dropped",
    "culled_entities",
    "lights_culled",
    "decals_visible",
    "decal_cluster_refs",
    "hiz_tested",
    "hiz_culled",
    "hiz_late",
    "cluster_tested",
    "cluster_culled",
    "render_passes",
    "compute_passes",
    "submits",
    "bind_groups_created",
    "buffer_writes",
];

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

/// A frame's clock speeds against the references, `None` where a probe missed.
#[derive(Clone, Copy, Default)]
struct Speeds {
    cpu: Option<f64>,
    gpu: Option<f64>,
}

fn time_probe(probe: Option<&ClockProbe>, harness: &mut Harness) -> Option<f64> {
    let probe = probe?;
    let renderer = harness.renderer()?;
    probe.time(renderer)
}

/// One frame's row values: the sim's CPU time and every system's share of it
/// (`cpu.<system>`, named by the schedule), what rendering it cost and each of its
/// stages' share (`render.<stage>`), its counters. Every GPU pass is sampled every
/// frame (zero when it did not run), so the rows are the same from run to run.
/// CPU and GPU times are scaled by the frame's clock `speed`s; a frame a probe
/// missed has no rows of that side's scaled times.
fn sample(samples: &mut Samples, stats: &FrameStats, frame: Option<&RenderedFrame>, speed: Speeds) {
    if let Some(s) = stats.get("frame_ms") {
        samples.push("frame_ms", s.last);
        if let Some(cpu) = speed.cpu {
            samples.push("cpu_ms", s.last * cpu);
            samples.push("cpu_clock_pct", cpu * 100.0);
            let systems = stats.tick_systems.iter().copied();
            push_parts(samples, "cpu", s.last, systems, cpu);
        }
    }
    let Some(frame) = frame else { return };
    samples.push("renderer_ms", frame.cpu_ms);
    let (stages, parts) = stage_rows(&frame.stages);
    push_parts(samples, "render", frame.cpu_ms, stages.iter().copied(), 1.0);
    for &(stage, _) in &stages {
        let of = parts
            .iter()
            .filter(|(p, _)| p.split_once('.').is_some_and(|(s, _)| s == stage));
        let rows = of.map(|&(p, ms)| (format!("render.{p}"), ms));
        samples.push_stage_parts(&format!("render.{stage}"), rows.collect::<Vec<_>>());
    }
    if let (Some(gpu), Some(speed)) = (&frame.gpu, speed.gpu) {
        samples.push("gpu_ms", gpu.total_ms() * speed);
        for pass in GpuPass::ALL {
            let ms = gpu.passes.iter().find(|(p, _)| *p == pass);
            let ms = ms.map_or(0.0, |m| m.1 * speed);
            samples.push(&format!("gpu.{}", pass.name()), ms);
        }
        push_gpu_parts(samples, &gpu.parts, speed);
        samples.push("gpu.other", gpu.other_ms * speed);
        // The GPU's wait for the CPU: as measured, a clock does not shorten it.
        samples.push("gpu_between_submits_ms", gpu.between_submits_ms);
        samples.push("gpu_raw_ms", gpu.total_ms());
        samples.push("gpu_clock_pct", speed * 100.0);
    }
    for (key, value) in frame.counters.pairs() {
        if COUNTERS.contains(&key) {
            samples.push(key, value as f64);
        }
    }
}

/// A pass's parts (#2401) as `gpu.<pass>.<part>` rows, inside their pass's row,
/// so no `gpu.<pass>` sum counts them. On a frame that timed any part of a pass,
/// every part of it is sampled, zero when absent (the cluster cull on a frame
/// with no clustered batch), so each part is averaged over the same frames and
/// viewpoints as its pass and the parts sum to it after the fold. A pass no
/// frame timed in parts (the cull off) has no part rows at all, on purpose: the
/// rows say the stage was split, a zero would say it was split and empty.
fn push_gpu_parts(samples: &mut Samples, parts: &[(GpuPart, f64)], speed: f64) {
    for part in GpuPart::ALL {
        if !parts.iter().any(|(p, _)| p.pass() == part.pass()) {
            continue;
        }
        let ms = parts.iter().find(|(p, _)| *p == part);
        samples.push(
            &format!("gpu.{}", part.row()),
            ms.map_or(0.0, |m| m.1 * speed),
        );
    }
}

/// Named CPU times, in order.
type Laps<'a> = Vec<(&'a str, f64)>;

/// The renderer's stages, each with its parts (`occlusion.select`, a lap named
/// `<stage>.<part>`, #2401) added in, and the parts on their own: a stage's row
/// stays its whole time, and the parts beside it sum to it but for what the
/// stage spent outside them.
fn stage_rows<'a>(laps: &[(&'a str, f64)]) -> (Laps<'a>, Laps<'a>) {
    let add = |rows: &mut Laps<'a>, name, ms| match rows.iter_mut().find(|(n, _)| *n == name) {
        Some((_, sum)) => *sum += ms,
        None => rows.push((name, ms)),
    };
    let (mut stages, mut parts) = (Vec::new(), Vec::new());
    for &(name, ms) in laps {
        let stage = name.split_once('.').map_or(name, |(stage, _)| stage);
        if stage != name {
            add(&mut parts, name, ms);
        }
        add(&mut stages, stage, ms);
    }
    (stages, parts)
}

/// A parent time's parts as `<prefix>.<part>` rows, each scaled by `scale`, and
/// `<prefix>.other`: what of `total` no part holds (timer overhead; a stage left
/// untimed shows here).
fn push_parts<'a>(
    samples: &mut Samples,
    prefix: &str,
    total: f64,
    parts: impl Iterator<Item = (&'a str, f64)>,
    scale: f64,
) {
    let mut sum = 0.0;
    for (name, ms) in parts {
        samples.push(&format!("{prefix}.{name}"), ms * scale);
        sum += ms;
    }
    samples.push(&format!("{prefix}.other"), (total - sum) * scale);
}

/// Measure `ticks` frames, print the report with its change against the last one
/// kept in `out_dir`, and keep this one there instead. Returns the report.
pub fn run(harness: &mut Harness, ticks: u32) -> Report {
    let mut report = measure(harness, ticks);
    let engine = rusty_core::project::engine_content::stamp();
    report.warnings.extend(engine.warning());
    report.engine = Some(engine);
    let out_dir = harness.out_dir().to_path_buf();
    let previous = read(&out_dir);
    report
        .warnings
        .extend(clock::cpu_drift(&report, previous.as_ref()));
    let mut text = report.render(previous.as_ref());
    if !report.metrics.iter().any(|(k, _)| k == "renderer_ms") {
        text.push_str("(no GPU or software adapter: nothing was rendered)\n");
    } else if !report.metrics.iter().any(|(k, _)| k == "gpu_ms") {
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
    println!("{text}");
    if let Err(e) = write(&out_dir, &report, &text) {
        harness.console.borrow_mut().error(format!("[Bench] {e}"));
    }
    harness.log(format!("bench: {} frames measured", report.frames));
    report
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "run_tests.rs"]
mod run_tests;

#[cfg(test)]
#[path = "query_cost_tests.rs"]
mod query_cost_tests;

#[cfg(test)]
#[path = "move_cost_tests.rs"]
mod move_cost_tests;
