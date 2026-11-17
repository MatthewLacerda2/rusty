//! crates/dev/src/bench/scenes/ — `make bench-scenes`: bench a game project's own
//! scenes, where `make bench` measures one synthetic stress yard.
//!
//! Each scene is loaded as Play loads it (its scripts run, with no input), drawn
//! once, then measured from deterministic viewpoints ([`views`]): each warms up
//! [`WARMUP`] frames (pipelines, uploads, shadow caches) and is measured for
//! [`FRAMES`] by `bench::measure_posed`, at 1920x1080 unless told otherwise. The
//! report ([`report`]) folds the viewpoints' rows into one ([`fold`]) and adds
//! `load_ms` (the scene's own load to its first rendered frame, wall clock, the
//! renderer's start-up off the clock: [`load`]), `rss_peak_mb` (the
//! process's peak resident memory, [`memory`]) and `gpu_mb` (the renderer's GPU
//! memory, `rusty_render::gpu_memory`), each compared with that scene's last run.
//!
//! The project is opened read-only and never seeded: nothing is written into it.
//! A signal, never a gate, like `make bench`. `bench-scenes` runs each scene in a
//! process of its own, so a scene's load time is a cold one and its memory peak
//! its own.

mod fold;
pub mod memory;
pub mod report;
pub mod views;

use std::path::Path;
use std::time::Instant;

use rusty_core::project::ProjectSettings;
use rusty_render::ClockProbe;
use rusty_scene::{Scene, Sh9};

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

/// Every scene the project at `root` holds: each `.scene` under its
/// `assets/scenes/`, subfolders included, named by its path there without the
/// extension (`levels/a`), sorted.
pub fn project_scenes(root: &Path) -> Vec<String> {
    let base = root.join("assets/scenes");
    let mut scenes = Vec::new();
    let mut dirs = vec![base.clone()];
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for path in entries.filter_map(|e| e.ok().map(|e| e.path())) {
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|x| x == "scene") {
                let name = path.strip_prefix(&base).unwrap_or(&path).with_extension("");
                let parts: Vec<_> = name.iter().map(|p| p.to_string_lossy()).collect();
                scenes.push(parts.join("/"));
            }
        }
    }
    scenes.sort();
    scenes
}

/// The folder a project's reports are kept in under the run's out dir: its
/// folder's name and a short hash of its canonical path, so a copy and its
/// original (one name, two places) keep histories of their own. `project` is
/// canonical already (`std::fs::canonicalize`: no `..`, no link).
pub fn out_key(project: &Path) -> String {
    let name = project
        .file_name()
        .map_or("project".into(), |n| n.to_string_lossy().into_owned());
    // FNV-1a: stable across runs and builds, unlike std's hasher.
    let hash = project
        .to_string_lossy()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("{name}-{:08x}", hash >> 32)
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
    let (mut harness, load_ms) = load(out_dir, &path, settings, opts.size)?;

    let probe = harness.renderer().and_then(|r| ClockProbe::new(r));
    let mut report = measure(&mut harness, probe.as_ref(), opts);
    drop(probe);
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
    {
        let world = harness.world.borrow();
        report.warnings.extend(unbaked(&world.scene().borrow()));
    }
    let engine = rusty_core::project::engine_content::stamp();
    report.warnings.extend(engine.warning());
    report.engine = Some(engine);

    let previous = report::read(out_dir, scene);
    let against = report.against(previous.as_ref());
    report.warnings.extend(against);
    let text = report.render(previous.as_ref());
    println!("{text}");
    report::write(out_dir, &report, &text).map_err(|e| format!("bench-scenes: {e}"))?;
    Ok(report)
}

/// Load the scene at `path` as Play does and draw its first frame at `size`,
/// (the harness's out dir `out_dir`), returning the harness and the load's wall-clock ms. The renderer starts (its
/// device and every pipeline, warm as a game's) between the scene's load and its
/// first frame, off the clock: what is timed is the scene's own load (its
/// document, sidecar and navmesh bake) and its first frame (uploads, the first
/// sim step), not the engine's start-up.
fn load(
    out_dir: &Path,
    path: &str,
    settings: ProjectSettings,
    size: (u32, u32),
) -> Result<(Harness, f64), String> {
    let start = Instant::now();
    let mut harness = Harness::on_project_scene(out_dir, path, settings)?;
    let loaded = start.elapsed();
    harness.set_size(size.0, size.1);
    if let Some(renderer) = harness.renderer() {
        renderer.warm_pipelines(false);
    }
    let start = Instant::now();
    harness.step(1);
    harness.render_frame();
    let load = loaded + start.elapsed();
    Ok((harness, load.as_secs_f64() * 1000.0))
}

/// Measure every viewpoint of the harness's scene, in order, into one report.
fn measure(harness: &mut Harness, probe: Option<&ClockProbe>, opts: Options) -> SceneReport {
    let views = {
        let world = harness.world.borrow();
        let scene = world.scene().borrow();
        let nav = world.resources.nav.borrow();
        views::pick(&scene, &nav)
    };
    let mut measured = Vec::new();
    let mut spreads = Vec::new();
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
        let (report, samples) = super::measure_posed(harness, opts.frames, probe, &mut pose);
        {
            let world = harness.world.borrow();
            views::restore(&mut world.scene().borrow_mut(), &turned_off);
        }
        spreads.push((viewpoint.label.clone(), super::clock::cpu_spreads(&samples)));
        measured.push((viewpoint.label.clone(), report));
    }
    let mut warnings = views.warnings;
    warnings.extend(fold::spread_warnings(&spreads));
    SceneReport {
        views: views.views.into_iter().map(|v| v.label).collect(),
        frames: opts.frames,
        rows: fold::aggregate(&measured),
        warnings,
        ..SceneReport::default()
    }
}

/// The warning for a scene benched without its baked lighting, naming what is
/// missing from what it places: no lightmaps; light probes placed whose SH never
/// loaded (every one zero: no sidecar, or one that failed); reflection probes
/// placed with no cube on disk. What a scene does not place is not missing.
pub fn unbaked(scene: &Scene) -> Option<String> {
    let mut missing = Vec::new();
    if !scene.lightmaps.holds_bake() {
        missing.push("lightmaps".to_string());
    }
    let probes = scene.probes.probes();
    if !probes.is_empty() && probes.iter().all(|p| p.sh == Sh9::zero()) {
        missing.push("light probes (placed, no baked SH loaded)".into());
    }
    let reflections = &scene.reflection_probes.probes;
    let no_cube = reflections
        .iter()
        .filter(|p| p.cubemap_path.is_empty() || !Path::new(&p.cubemap_path).is_file())
        .count();
    if no_cube > 0 {
        missing.push(format!(
            "reflection probes ({no_cube} of {} with no cube on disk)",
            reflections.len()
        ));
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
