use rusty_core::frame_stats::{is_counter, FrameStats};
use rusty_render::{GpuPart, GpuPass, RenderCounters};

use super::Samples;
use crate::capture::RenderedFrame;

pub(super) const SHOWN: [&str; 18] = [
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

#[derive(Clone, Copy, Default)]
pub(super) struct Speeds {
    pub(super) cpu: Option<f64>,
    pub(super) gpu: Option<f64>,
}

pub(super) fn sample(
    samples: &mut Samples,
    stats: &FrameStats,
    frame: Option<(&RenderedFrame, f64)>,
    speed: Speeds,
) {
    if let Some(s) = stats.get("frame_ms") {
        samples.push("frame_ms", s.last);
        if let Some(cpu) = speed.cpu {
            samples.push("cpu_ms", s.last * cpu);
            samples.push("cpu_clock_pct", cpu * 100.0);
            let systems = stats.tick_systems.iter().copied();
            push_parts(samples, "cpu", s.last, systems, cpu);
            let beside = ["taps_ms", "probe_ms"].into_iter();
            beside.for_each(|k| _ = stats.get(k).inspect(|t| samples.push(k, t.last * cpu)));
        }
    }
    if let Some((frame, wall_ms)) = frame {
        sample_render(samples, frame, wall_ms, speed.gpu);
    }
    if stats.get("frame_ms").is_some() {
        push_tick_counters(samples, stats);
    }
}

fn push_tick_counters(samples: &mut Samples, stats: &FrameStats) {
    let render = RenderCounters::default().pairs();
    let rendered = |key: &str| render.iter().any(|(k, _)| *k == key);
    let earlier = samples.values("frame_ms").len().saturating_sub(1);
    for (key, s) in &stats.metrics {
        if is_counter(key) && !rendered(key) {
            if samples.values(key).is_empty() {
                (0..earlier).for_each(|_| push_counter(samples, key, 0.0));
            }
            push_counter(samples, key, s.last);
        }
    }
}

fn push_counter(samples: &mut Samples, key: &str, value: f64) {
    if SHOWN.contains(&key) {
        samples.push(key, value);
    } else {
        samples.push_quiet(key, value);
    }
}

pub(super) fn sample_render(
    samples: &mut Samples,
    frame: &RenderedFrame,
    wall_ms: f64,
    gpu_speed: Option<f64>,
) {
    samples.push("renderer_ms", frame.cpu_ms);
    samples.push("wall_ms", wall_ms);
    let (stages, parts) = stage_rows(&frame.stages);
    push_parts(samples, "render", frame.cpu_ms, stages.iter().copied(), 1.0);
    for &(stage, _) in &stages {
        let of = parts
            .iter()
            .filter(|(p, _)| p.split_once('.').is_some_and(|(s, _)| s == stage));
        let rows = of.map(|&(p, ms)| (format!("render.{p}"), ms));
        samples.push_stage_parts(&format!("render.{stage}"), rows.collect::<Vec<_>>());
    }
    if let (Some(gpu), Some(speed)) = (&frame.gpu, gpu_speed) {
        samples.push("gpu_ms", gpu.total_ms() * speed);
        for pass in GpuPass::ALL {
            let ms = gpu.passes.iter().find(|(p, _)| *p == pass);
            let ms = ms.map_or(0.0, |m| m.1 * speed);
            samples.push(&format!("gpu.{}", pass.name()), ms);
        }
        push_gpu_parts(samples, &gpu.parts, speed);
        samples.push("gpu.other", gpu.other_ms * speed);
        samples.push("gpu_between_submits_ms", gpu.between_submits_ms);
        samples.push("gpu_raw_ms", gpu.total_ms());
        samples.push("gpu_clock_pct", speed * 100.0);
    }
    for (key, value) in frame.counters.pairs() {
        if is_counter(key) {
            push_counter(samples, key, value as f64);
        }
    }
}

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

type Laps<'a> = Vec<(&'a str, f64)>;

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

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
