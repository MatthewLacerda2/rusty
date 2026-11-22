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

pub fn measure(harness: &mut Harness, ticks: u32) -> Report {
    let probe = harness.renderer().and_then(|r| ClockProbe::new(r));
    let (mut report, samples) = measure_posed(harness, ticks, probe.as_ref(), &mut |_| {});
    report.warnings = clock::cpu_warnings(&samples);
    report
}

pub(crate) fn measure_posed(
    harness: &mut Harness,
    ticks: u32,
    probe: Option<&ClockProbe>,
    pose: &mut dyn FnMut(&mut Harness),
) -> (Report, Samples) {
    let mut samples = Samples::default();
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

const BESIDE: [&str; 3] = ["loads_ms", "taps_ms", "probe_ms"];

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

fn beside_counts(stats: &FrameStats) -> (u64, [u64; 3]) {
    let samples = BESIDE.map(|k| stats.get(k).map_or(0, |s| s.samples));
    (stats.frames, samples)
}

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
