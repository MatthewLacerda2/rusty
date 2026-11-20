//! One measured frame's rows (#835): the tick's CPU time and its systems', the
//! renderer's by stage, the GPU passes and the render counters, each scaled by the
//! frame's clock speeds (`clock`). `make bench`, `make bench-scenes`' viewpoints and
//! its editor round (`scenes::editor`) all sample through here.

use rusty_core::frame_stats::FrameStats;
use rusty_render::{GpuPart, GpuPass};

use super::Samples;
use crate::capture::RenderedFrame;

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

/// A frame's clock speeds against the references, `None` where a probe missed.
#[derive(Clone, Copy, Default)]
pub(super) struct Speeds {
    pub(super) cpu: Option<f64>,
    pub(super) gpu: Option<f64>,
}

/// One frame's row values: the tick's CPU time, every system's share of it
/// (`cpu.<system>`, `flush_bounds` among them), a demo's `taps_ms` beside it, what
/// rendering cost and each stage's share (`render.<stage>`), its counters. Every GPU
/// pass is sampled every frame (zero when it did not run), so the rows are stable.
/// CPU and GPU times are scaled by the frame's clock `speed`s; a frame a probe
/// missed has no rows of that side's scaled times.
pub(super) fn sample(
    samples: &mut Samples,
    stats: &FrameStats,
    frame: Option<&RenderedFrame>,
    speed: Speeds,
) {
    if let Some(s) = stats.get("frame_ms") {
        samples.push("frame_ms", s.last);
        if let Some(cpu) = speed.cpu {
            samples.push("cpu_ms", s.last * cpu);
            samples.push("cpu_clock_pct", cpu * 100.0);
            let systems = stats.tick_systems.iter().copied();
            push_parts(samples, "cpu", s.last, systems, cpu);
            let taps = stats.get("taps_ms"); // beside the tick, not in it
            taps.inspect(|t| samples.push("taps_ms", t.last * cpu));
        }
    }
    if let Some(frame) = frame {
        sample_render(samples, frame, speed.gpu);
    }
}

/// A rendered frame's rows: the renderer's CPU time and each stage's share
/// (`render.<stage>`), the GPU passes at the clock `gpu_speed` (none where its
/// probe missed), its counters. The editor round samples its frames with it
/// (`scenes::editor`).
pub(super) fn sample_render(samples: &mut Samples, frame: &RenderedFrame, gpu_speed: Option<f64>) {
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
    if let (Some(gpu), Some(speed)) = (&frame.gpu, gpu_speed) {
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
/// `<prefix>.other`: what of `total` no part holds (for `render.`, timer overhead
/// and a stage left untimed; for `cpu.`, the tick's own work outside its systems).
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
