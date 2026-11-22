//! `make bench-scenes`, as `docs/testing.md`, *A project's own scenes*, describes it.

pub mod args;
pub mod editor;
mod fold;
pub mod import;
pub mod load;
mod measure;
mod notes;
pub mod profile;
pub mod report;
pub mod set;
pub mod views;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rusty_core::project::ProjectSettings;
use rusty_render::ClockProbe;

pub use load::{load, Load, LOAD_RUNS};
pub use notes::{unbaked, uncompressed};
pub use report::SceneReport;

pub const DEFAULT_SIZE: (u32, u32) = (1920, 1080);
// Frames a viewpoint draws: `WARMUP` unmeasured, keeping first-draw costs out, then `FRAMES`.
pub const WARMUP: u32 = 60;
pub const FRAMES: u32 = 300;
pub const RUNS: u32 = 3;

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub size: (u32, u32),
    pub warmup: u32,
    pub frames: u32,
    pub runs: u32,
    /// Build every lazy pipeline before the first frame, off the clock, as a game's window
    /// does. Only tests turn it off, where no frame's cost is read.
    pub warm: bool,
    pub switches: rusty_render::RenderSwitches,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            warmup: WARMUP,
            frames: FRAMES,
            runs: RUNS,
            warm: true,
            switches: rusty_render::RenderSwitches::ON,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Extra {
    pub baseline: Option<PathBuf>,
    pub key: String,
    pub import_ms: Option<f64>,
    pub loads: Vec<Load>,
    pub set: Vec<String>,
    pub editor: Option<editor::EditorRound>,
}

pub fn open(path: &Path, set: &set::Overrides) -> Result<rusty_core::project::Opened, String> {
    let mut opened = rusty_core::project::open(path, rusty_core::project::Access::Run)?;
    set.apply_settings(&mut opened.settings)?;
    Ok(opened)
}

pub fn project_scenes(root: &Path) -> Vec<String> {
    let base = root.join("assets/scenes");
    let mut scenes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut dirs = vec![base.clone()];
    while let Some(dir) = dirs.pop() {
        let Ok(real) = std::fs::canonicalize(&dir) else {
            continue;
        };
        if !seen.insert(real) {
            continue;
        }
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
    scenes.dedup();
    scenes
}

/// `project` must be canonical (`std::fs::canonicalize`), or one project gets two histories.
pub fn out_key(project: &Path) -> String {
    let name = project
        .file_name()
        .map_or("project".into(), |n| n.to_string_lossy().into_owned());
    // FNV-1a: stable across runs and builds, unlike std's hasher, so a project keeps its folder.
    let hash = project
        .to_string_lossy()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
        });
    format!("{name}-{:08x}", hash >> 32)
}

pub fn bench(
    root: &Path,
    scene: &str,
    settings: ProjectSettings,
    out_dir: &Path,
    opts: Options,
    extra: &Extra,
) -> Result<SceneReport, String> {
    let (mut harness, own) = load(out_dir, root, scene, settings, opts)?;
    // Read before the measured ticks: a static the game moves later is the game's, not a stale
    // bake.
    let loaded = harness.world.borrow().scene().borrow().id();
    let stale = harness.renderer().and_then(|r| r.stale_occlusion(loaded));
    let lights = notes::lights(&harness.world.borrow().scene().borrow());

    let probe = harness.renderer().and_then(|r| ClockProbe::new(r));
    let mut report = measure::all(&mut harness, probe.as_ref(), opts);
    drop(probe);
    report.scene = scene.to_string();
    report.size = opts.size;
    report.set.clone_from(&extra.set);
    if crate::stats::verbs::armed() {
        report.script = true;
        super::verbs::keep_top(&mut report.rows);
    }
    handed(&mut report, extra, own);
    if let Some(bytes) = crate::memory::rss_peak_bytes() {
        report.push_value("rss_peak_mb", mb(bytes));
    }
    if let Some(r) = harness.renderer() {
        push_gpu_memory(&mut report, r);
    }
    notes::push_lights(&mut report, lights);
    let own = root.join(format!("assets/scenes/{scene}.scene"));
    report
        .warnings
        .extend(notes::scene_bakes(&harness, &own, loaded, stale));
    let assets = std::path::Path::new(rusty_core::project::ASSETS_DIR);
    let uncached = harness.renderer().map(|r| r.uncached_textures(assets));
    report.warnings.extend(uncached.and_then(uncompressed));
    let console = notes::keep_console(&harness, out_dir, scene);
    report.warnings.extend(console);
    let engine = rusty_core::project::engine_content::stamp();
    report.warnings.extend(engine.warning());
    report.engine = Some(engine);

    let previous = match &extra.baseline {
        Some(dir) => {
            report.baseline = Some(dir.display().to_string());
            report::read_baseline(dir, &extra.key, scene, report.script)
        }
        None => report::read(out_dir, scene),
    };
    let against = report.against(previous.as_ref());
    report.warnings.extend(against);
    let text = report.render(previous.as_ref());
    println!("{text}");
    report::write(out_dir, &report, &text).map_err(|e| format!("bench-scenes: {e}"))?;
    Ok(report)
}

fn handed(report: &mut SceneReport, extra: &Extra, own: Load) {
    let loads: Vec<Load> = extra.loads.iter().cloned().chain([own]).collect();
    report.loads = loads.len() as u32;
    report.rows.extend(load::rows(&loads));
    report.warnings.extend(notes::cooked_at_load(&loads));
    if let Some(ms) = extra.import_ms {
        report.push_value("import_ms", ms);
    }
    if let Some(round) = &extra.editor {
        report.rows.extend(round.rows.iter().cloned());
        report.warnings.extend(round.warnings.iter().cloned());
    }
}

fn push_gpu_memory(report: &mut SceneReport, r: &rusty_render::Renderer) {
    if let Some(gpu) = rusty_render::gpu_memory(&r.device) {
        report.push_value("gpu_mb", mb(gpu.bytes));
        if let Some(bytes) = gpu.allocated {
            report.push_value("gpu_allocated_mb", mb(bytes));
        }
        report.gpu_mb_source = Some(gpu.source.to_string());
    }
    push_resident(report, &r.frame_counters);
}

fn push_resident(report: &mut SceneReport, counters: &rusty_render::RenderCounters) {
    let kinds = [
        ("gpu_texture_mb", counters.texture_bytes),
        ("gpu_lightmap_mb", counters.lightmap_bytes),
        ("gpu_reflection_mb", counters.reflection_bytes),
        ("gpu_mesh_mb", counters.mesh_bytes),
        ("gpu_cluster_pool_mb", counters.cluster_pool_bytes),
    ];
    for (row, bytes) in kinds {
        report.push_value(row, mb(bytes));
    }
}

fn mb(bytes: u64) -> f64 {
    (bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0
}

#[cfg(test)]
#[path = "baked_gpu_tests.rs"]
mod baked_gpu_tests;
#[cfg(test)]
#[path = "run_tests.rs"]
mod run_tests;

#[cfg(test)]
#[path = "rows_tests.rs"]
mod rows_tests;

#[cfg(test)]
#[path = "set_gpu_tests.rs"]
mod set_gpu_tests;
