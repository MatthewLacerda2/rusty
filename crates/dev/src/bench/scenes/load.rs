//! A scene's load, timed step by step and repeated: the `load_ms`, `load_cold_ms`,
//! `load.<step>` and `load.count.<what>` rows of `make bench-scenes`.
//!
//! **Steps.** The boot load runs each step under a [`StepClock`] through
//! `rusty_app::timed`, the one implementation a play-mode `Scene.Load` times its
//! own steps with: the harness's `document`, `import`, `apply`, `navmesh` and
//! `world` (`Harness::on_project_scene_timed`), then the first tick's Play entry,
//! `refresh`, `scripts`, `physics` and `startup` (`GameWorld::set_entry_probe`),
//! `tick` (the rest of that first tick: the entry's untimed bookkeeping and the
//! first sim step) and `frame` (the first frame: its uploads and draw). They tile
//! the load, so they sum to its `load_ms`; the renderer's start-up (its device and
//! warm pipelines) sits between `world` and the first tick, off the clock.
//!
//! **Repeats.** Each load runs in a fresh process (`bench-scenes` spawns
//! `--load-out` children before the bench's own, `--load-runs` in all). The
//! first is the cold one, the scene's files not yet in the page cache, and is
//! reported apart as `load_cold_ms`; `load_ms` and the steps are the warm loads'
//! average with their spread ([`rows`]), so a load change can leave the noise.
//! A run of one load has no warm load: its one load stands as `load_ms`, as cold.
//!
//! **Counts.** What the bench's own load did, which tells a cached load from a
//! cold import: model files parsed into the import cache, the textures resident
//! after the first frame compressed and decoded, and the MB the first frame
//! uploaded. Deterministic for a project as it stands, so no spread.
//!
//! Wall-clock timing in the dev bench: nothing here reaches the sim.
//!
//! stored-format: `<out>/<project>-<hash>/.loads/<n>.json`, a scene's earlier loads handed to its bench process, deleted after

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::Instant;

use rusty_app::LoadProbe;
use rusty_core::project::ProjectSettings;
use rusty_script::scripting::ConsoleLogs;
use serde::{Deserialize, Serialize};

use super::super::super::harness::Harness;
use super::fold::over_runs;
use super::report::Row;
use super::{mb, Options};

/// How many times a scene is loaded unless told otherwise (`LOAD_RUNS`).
pub const LOAD_RUNS: u32 = 3;

/// One load of a scene: its steps' ms in the order they ran, then its counts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Load {
    pub steps: Vec<(String, f64)>,
    pub counts: Vec<(String, f64)>,
}

impl Load {
    /// The load's wall-clock ms: its steps' sum, which tile it.
    pub fn total_ms(&self) -> f64 {
        self.steps.iter().map(|(_, ms)| ms).sum()
    }

    /// `load_ms` then each step's `load.<step>`, as one run's rows.
    fn timed_rows(&self) -> Vec<Row> {
        let steps = self.steps.iter().map(|(s, ms)| (format!("load.{s}"), *ms));
        let all = std::iter::once(("load_ms".to_string(), self.total_ms())).chain(steps);
        all.map(|(metric, ms)| one(metric, ms)).collect()
    }
}

/// The load rows of `loads`, in the order they ran (the bench's own last):
/// `load_ms` and the `load.<step>` rows over the warm loads with their spread,
/// `load_cold_ms` (the first) when there were two or more, then the last load's
/// `load.count.<what>` rows.
pub fn rows(loads: &[Load]) -> Vec<Row> {
    let Some(last) = loads.last() else {
        return Vec::new();
    };
    let warm = if loads.len() > 1 { &loads[1..] } else { loads };
    let mut rows = over_runs(&warm.iter().map(Load::timed_rows).collect::<Vec<_>>());
    if loads.len() > 1 {
        rows.insert(1, one("load_cold_ms".into(), loads[0].total_ms()));
    }
    let counts = last.counts.iter();
    rows.extend(counts.map(|(c, n)| one(format!("load.count.{c}"), *n)));
    rows
}

/// Keep `load` at `file`, for the bench that folds it (`--load-out`).
pub fn write(file: &Path, load: &Load) -> Result<(), String> {
    let json = serde_json::to_string(load).map_err(|e| e.to_string())?;
    // lint: allow(asset-write) a load handed to the scene's bench process, deleted after
    std::fs::write(file, json).map_err(|e| format!("{}: {e}", file.display()))
}

/// The loads kept in `dir` (`--loads`), in the order they ran: `0.json`, `1.json`, …
pub fn read_all(dir: &Path) -> Result<Vec<Load>, String> {
    let mut loads = Vec::new();
    while let Ok(json) = std::fs::read_to_string(dir.join(format!("{}.json", loads.len()))) {
        let load = serde_json::from_str(&json).map_err(|e| format!("{}: {e}", dir.display()))?;
        loads.push(load);
    }
    Ok(loads)
}

/// A one-off row of `value`.
fn one(metric: String, value: f64) -> Row {
    let (avg, p95, lo, hi) = (value, value, value, value);
    let worst = String::new();
    Row {
        metric,
        avg,
        p95,
        worst,
        lo,
        hi,
    }
}

/// Records each step it runs, in order, with its wall-clock ms.
#[derive(Clone, Default)]
pub struct StepClock(Rc<RefCell<Vec<(String, f64)>>>);

impl StepClock {
    fn push(&self, name: &str, ms: f64) {
        self.0.borrow_mut().push((name.to_string(), ms));
    }

    /// The ms of the steps recorded from the `from`th on.
    fn since(&self, from: usize) -> f64 {
        self.0.borrow()[from..].iter().map(|(_, ms)| ms).sum()
    }
}

impl LoadProbe for StepClock {
    fn step(&mut self, name: &'static str, step: &mut dyn FnMut()) {
        let start = Instant::now();
        step();
        self.push(name, ms_since(start));
    }

    fn finish(&mut self, _: Option<&str>, _: &mut ConsoleLogs) {}
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// Load `scene` of the project at `root` as Play does and draw its first frame
/// at `opts.size` (the harness's out dir `out_dir`), returning the harness and the
/// load, each step timed (the module's *Steps*). The renderer starts (its device
/// and, with `opts.warm`, every pipeline, warm as a game's) between the boot
/// load and the first tick, off the clock.
pub fn load(
    out_dir: &Path,
    root: &Path,
    scene: &str,
    settings: ProjectSettings,
    opts: Options,
) -> Result<(Harness, Load), String> {
    let path = root.join(format!("assets/scenes/{scene}.scene"));
    let clock = StepClock::default();
    let parsed = rusty_asset::import_cache::parses();
    let mut probe: Option<Box<dyn LoadProbe>> = Some(Box::new(clock.clone()));
    let path = path.to_string_lossy();
    let mut harness = Harness::on_project_scene_timed(out_dir, &path, settings, &mut probe)?;
    harness.set_size(opts.size.0, opts.size.1);
    if let Some(renderer) = harness.renderer().filter(|_| opts.warm) {
        renderer.warm_pipelines(false);
    }
    let entry = clock.0.borrow().len();
    harness.world.borrow_mut().set_entry_probe(probe);
    let start = Instant::now();
    harness.step(1);
    let tick = ms_since(start);
    harness.world.borrow_mut().set_entry_probe(None);
    clock.push("tick", (tick - clock.since(entry)).max(0.0));
    let start = Instant::now();
    harness.render_frame();
    clock.push("frame", ms_since(start));
    let parsed = rusty_asset::import_cache::parses().saturating_sub(parsed);
    let mut counts = vec![("models_parsed".to_string(), parsed as f64)];
    counts.extend(harness.renderer().map(render_counts).into_iter().flatten());
    let steps = clock.0.take();
    Ok((harness, Load { steps, counts }))
}

/// The textures resident after the first frame, compressed and decoded, and the
/// MB that frame uploaded.
fn render_counts(r: &mut rusty_render::Renderer) -> [(String, f64); 3] {
    let textures = r.gpu_textures.values();
    let compressed = textures
        .filter(|t| t.texture.format().is_compressed())
        .count();
    let decoded = r.gpu_textures.len() - compressed;
    [
        ("textures_compressed".into(), compressed as f64),
        ("textures_decoded".into(), decoded as f64),
        ("upload_mb".into(), mb(r.frame_counters.uploads.bytes)),
    ]
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
