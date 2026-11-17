//! crates/dev/src/bench/scenes/ — `make bench-scenes`: bench a game project's own
//! scenes, where `make bench` measures one synthetic stress yard.
//!
//! Each scene is loaded as Play loads it (its scripts run, with no input), drawn
//! once, then measured from deterministic viewpoints ([`views`]): each warms up
//! [`WARMUP`] frames (pipelines, uploads, shadow caches) and is measured for
//! [`FRAMES`] by [`super::measure_posed`], at 1920x1080 unless told otherwise. The
//! report ([`report`]) aggregates the viewpoints' rows and adds `load_ms` (the
//! scene's load to its first rendered frame, wall clock), `rss_peak_mb` (the
//! process's peak resident memory, [`memory`]) and `gpu_mb` (the renderer's GPU
//! memory, `rusty_render::gpu_memory`), each compared with that scene's last run.
//!
//! The project is opened read-only and never seeded: nothing is written into it.
//! A signal, never a gate, like `make bench`. `bench-scenes` runs each scene in a
//! process of its own, so a scene's load time is a cold one and its memory peak
//! its own.

pub mod memory;
pub mod report;
pub mod views;

use std::path::Path;
use std::time::Instant;

use rusty_core::project::ProjectSettings;

use super::super::harness::Harness;
pub use report::SceneReport;

/// The size a scene is benched at unless told otherwise.
pub const DEFAULT_SIZE: (u32, u32) = (1920, 1080);
/// Frames drawn at each viewpoint before it is measured.
pub const WARMUP: u32 = 60;
/// Frames measured at each viewpoint.
pub const FRAMES: u32 = 300;

/// How a scene is benched.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub size: (u32, u32),
    pub warmup: u32,
    pub frames: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            warmup: WARMUP,
            frames: FRAMES,
        }
    }
}

/// Every scene the project at `root` holds: its `assets/scenes/*.scene` stems, sorted.
pub fn project_scenes(root: &Path) -> Vec<String> {
    let Ok(dir) = std::fs::read_dir(root.join("assets/scenes")) else {
        return Vec::new();
    };
    let mut scenes: Vec<String> = dir
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "scene"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    scenes.sort();
    scenes
}

/// Bench the scene `scene` (`assets/scenes/<scene>.scene`) of the project at
/// `root` (opened read-only, so the working directory), print its table and keep
/// its report in `out_dir`, beside the last one it is compared with.
pub fn bench(
    root: &Path,
    scene: &str,
    settings: ProjectSettings,
    out_dir: &Path,
    opts: Options,
) -> Result<SceneReport, String> {
    let path = root.join(format!("assets/scenes/{scene}.scene"));
    let path = path.to_string_lossy();
    let start = Instant::now();
    let mut harness = Harness::on_project_scene(out_dir, &path, settings)?;
    harness.set_size(opts.size.0, opts.size.1);
    harness.step(1);
    harness.render_frame();
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut report = measure(&mut harness, opts);
    report.scene = scene.to_string();
    report.size = opts.size;
    report.push_value("load_ms", load_ms);
    if let Some(bytes) = memory::rss_peak_bytes() {
        report.push_value("rss_peak_mb", mb(bytes));
    }
    if let Some(gpu) = harness
        .renderer()
        .and_then(|r| rusty_render::gpu_memory(&r.device))
    {
        report.push_value("gpu_mb", mb(gpu.bytes));
        report.gpu_mb_source = Some(gpu.source.to_string());
    }
    report.warnings.extend(unbaked(&harness));
    let engine = rusty_core::project::engine_content::stamp();
    report.warnings.extend(engine.warning());
    report.engine = Some(engine);

    let previous = report::read(out_dir, scene);
    let text = report.render(previous.as_ref());
    println!("{text}");
    report::write(out_dir, &report, &text).map_err(|e| format!("bench-scenes: {e}"))?;
    Ok(report)
}

/// Measure every viewpoint of the harness's scene, in order, into one report.
fn measure(harness: &mut Harness, opts: Options) -> SceneReport {
    let views = {
        let world = harness.world.borrow();
        let scene = world.scene().borrow();
        let nav = world.resources.nav.borrow();
        views::pick(&scene, &nav)
    };
    let mut measured = Vec::new();
    for viewpoint in &views.views {
        let mut turned_off = Vec::new();
        let mut pose = |h: &mut Harness| {
            let world = h.world.borrow();
            let mut scene = world.scene().borrow_mut();
            let mut camera = world.resources.camera.borrow_mut();
            turned_off.extend(views::apply(&mut scene, &mut camera, &viewpoint.view));
        };
        for _ in 0..opts.warmup {
            harness.step(1);
            pose(harness);
            harness.render_frame();
        }
        let report = super::measure_posed(harness, opts.frames, &mut pose);
        {
            let world = harness.world.borrow();
            views::restore(&mut world.scene().borrow_mut(), &turned_off);
        }
        measured.push((viewpoint.label.clone(), report));
    }
    let mut warnings = views.warnings;
    for (label, report) in &measured {
        warnings.extend(report.warnings.iter().map(|w| format!("{label}: {w}")));
    }
    SceneReport {
        views: views.views.into_iter().map(|v| v.label).collect(),
        frames: opts.frames,
        rows: report::aggregate(&measured),
        warnings,
        ..SceneReport::default()
    }
}

/// The warning for a scene benched without its baked lighting, naming what is missing.
fn unbaked(harness: &Harness) -> Option<String> {
    let world = harness.world.borrow();
    let scene = world.scene().borrow();
    let mut missing = Vec::new();
    if !scene.lightmaps.holds_bake() {
        missing.push("lightmaps");
    }
    if scene.probes.is_empty() {
        missing.push("light probes");
    }
    let reflections = scene.reflection_probes.probes.iter();
    if !reflections.clone().any(|p| !p.cubemap_path.is_empty()) {
        missing.push("reflection probes");
    }
    (!missing.is_empty()).then(|| {
        format!(
            "not baked: {} (its lighting cost is not the shipped one)",
            missing.join(", ")
        )
    })
}

fn mb(bytes: u64) -> f64 {
    (bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod run_tests;
