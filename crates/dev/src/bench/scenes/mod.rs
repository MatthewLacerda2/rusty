//! crates/dev/src/bench/scenes/ — `make bench-scenes`: bench a game project's own
//! scenes, where `make bench` measures one synthetic stress yard.
//!
//! Each scene is loaded as Play loads it (its scripts run, with no input), drawn
//! once, then measured from deterministic viewpoints ([`views`]): each warms up
//! [`WARMUP`] frames (pipelines, uploads, shadow caches) and is measured for
//! [`FRAMES`] by `bench::measure_posed`, at 1920x1080 unless told otherwise, and
//! the whole round is run [`Options::runs`] times (`measure`). The report
//! ([`report`]) folds the viewpoints' rows, then the runs', into one (`fold`),
//! each row with its run-to-run spread, and adds `load_ms` (the scene's load to
//! its first rendered frame, wall clock, the renderer's start-up off the clock,
//! repeated in fresh processes with its spread, the run's first load apart as
//! `load_cold_ms`, its steps as `load.<step>` and what it did as
//! `load.count.<what>`: [`load`](mod@load)), `import_ms` (`IMPORT=1`, once a run, on its first scene: the
//! same load of a copy of the project without its import cache, [`import`]), `rss_peak_mb` (the process's peak resident
//! memory, [`crate::memory`]), `gpu_mb` (the renderer's GPU memory,
//! `rusty_render::gpu_memory`, with `gpu_allocated_mb`, what live allocations use,
//! where the allocator tells it) and `gpu_texture_mb`, `gpu_lightmap_mb` and
//! `gpu_reflection_mb` (the renderer's own resident bytes by kind), each compared with that scene's last run or a
//! kept baseline (`Extra::baseline`). A run with `PROFILE=1` adds a sampling
//! profile of the measured frames ([`profile`]), in a run of its own. The editor
//! round ([`editor`], on unless `EDITOR=0`) adds the `editor.` frame rows and
//! `play_enter_ms`, `play_stop_ms`, `play_rss_mb` and `save_ms`, measured in a
//! process of their own.
//!
//! The project is opened read-only and never seeded: nothing is written into it.
//! A signal, never a gate, like `make bench`. `bench-scenes` runs each scene in a
//! process of its own, so its memory peak is its own, and each of its loads in
//! one too ([`load`](mod@load)).

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

/// The size a scene is benched at unless told otherwise.
pub const DEFAULT_SIZE: (u32, u32) = (1920, 1080);
/// Frames drawn at each viewpoint before it is measured.
pub const WARMUP: u32 = 60;
/// Frames measured at each viewpoint.
pub const FRAMES: u32 = 300;
/// How many times each scene's viewpoints are measured unless told otherwise.
pub const RUNS: u32 = 3;

/// How a scene is benched.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub size: (u32, u32),
    pub warmup: u32,
    pub frames: u32,
    /// Rounds over every viewpoint, each one's averages a point of the spread.
    pub runs: u32,
    /// Build every lazy pipeline before the first frame, off the clock, as a
    /// game's window does (`Renderer::warm_pipelines`): on unless the frames'
    /// costs are not what is read. Off, a pipeline compiles on its first draw
    /// inside the load or a warm-up frame; on lavapipe the warm-up is most of a
    /// small bench's time (#2350).
    pub warm: bool,
    /// The renderer's measurement switches (`--set render.<switch>`, [`set`]),
    /// given to every renderer the bench builds before its first frame.
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

/// What a scene's bench is compared with and told beside its own run.
#[derive(Clone, Debug, Default)]
pub struct Extra {
    /// A kept report folder to compare with instead of the last run (`BEFORE`).
    pub baseline: Option<PathBuf>,
    /// The project's key under the out dir ([`out_key`]), to find it in a
    /// baseline that kept every project's folder.
    pub key: String,
    /// The cold load an import run measured (`import_ms`), when one ran.
    pub import_ms: Option<f64>,
    /// The scene's loads before the bench's own, each in a process of its own
    /// (`--loads`), the first the cold one ([`load`](mod@load)).
    pub loads: Vec<Load>,
    /// The run's `--set` overrides, as given (`set::Overrides::said`), which the
    /// report records.
    pub set: Vec<String>,
    /// The editor round's rows, measured in a process of its own (`--editor`,
    /// [`editor`]), when one ran.
    pub editor: Option<editor::EditorRound>,
}

/// The project at `path` opened to run, `set` written over its running preset
/// and its graphics option defaults (its `render.` switches go in [`Options`]).
pub fn open(path: &Path, set: &set::Overrides) -> Result<rusty_core::project::Opened, String> {
    let mut opened = rusty_core::project::open(path, rusty_core::project::Access::Run)?;
    set.apply_settings(&mut opened.settings)?;
    Ok(opened)
}

/// Every scene the project at `root` holds: each `.scene` under its
/// `assets/scenes/`, subfolders included, named by its path there without the
/// extension (`levels/a`), sorted. A folder linked twice (or a link back up the
/// tree) is walked once.
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
/// its report in `out_dir`, beside the last one it is compared with (or the
/// baseline's, `extra`), and the bench's console beside it.
pub fn bench(
    root: &Path,
    scene: &str,
    settings: ProjectSettings,
    out_dir: &Path,
    opts: Options,
    extra: &Extra,
) -> Result<SceneReport, String> {
    let (mut harness, own) = load(out_dir, root, scene, settings, opts)?;
    // Judged at the load's first frame, before a gameplay tick: a static the game
    // then moves or destroys is the game's doing, not a stale bake.
    let loaded = harness.world.borrow().scene().borrow().id();
    let stale = harness.renderer().and_then(|r| r.stale_occlusion(loaded));

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

/// What the scene's other children measured, into its report: its loads (`own`
/// the bench's, the last), the run's `import_ms` and the editor round.
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

/// `gpu_mb` (and `gpu_allocated_mb` where the source tells it) with its source,
/// then the renderer's own resident bytes by kind.
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

/// The renderer's own resident GPU bytes by kind, as of the last measured frame,
/// beside `gpu_mb` (which no scene's size moves, being the allocator's blocks).
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
