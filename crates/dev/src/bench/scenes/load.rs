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

pub const LOAD_RUNS: u32 = 3;

const CONVERTED: &str = "models_converted";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Load {
    pub steps: Vec<(String, f64)>,
    pub counts: Vec<(String, f64)>,
    /// The screen cameras live before Play's first tick (`views::screen_cameras`): the
    /// scene's authored viewpoints. Only the process that loaded reads it.
    #[serde(skip)]
    pub cameras: Vec<u32>,
}

impl Load {
    pub fn total_ms(&self) -> f64 {
        self.steps.iter().map(|(_, ms)| ms).sum()
    }

    fn count(&self, what: &str) -> Option<f64> {
        self.counts.iter().find(|(c, _)| c == what).map(|(_, n)| *n)
    }

    fn timed_rows(&self) -> Vec<Row> {
        let steps = self.steps.iter().map(|(s, ms)| (format!("load.{s}"), *ms));
        let all = std::iter::once(("load_ms".to_string(), self.total_ms())).chain(steps);
        all.map(|(metric, ms)| Row::value(metric, ms)).collect()
    }
}

pub fn rows(loads: &[Load]) -> Vec<Row> {
    let Some(last) = loads.last() else {
        return Vec::new();
    };
    let warm = if loads.len() > 1 { &loads[1..] } else { loads };
    let mut rows = over_runs(&warm.iter().map(Load::timed_rows).collect::<Vec<_>>());
    if loads.len() > 1 {
        rows.insert(1, Row::value("load_cold_ms".into(), loads[0].total_ms()));
    }
    for (c, n) in &last.counts {
        let n = match c.as_str() {
            CONVERTED => loads.iter().filter_map(|l| l.count(CONVERTED)).sum(),
            _ => *n,
        };
        rows.push(Row::value(format!("load.count.{c}"), n));
    }
    rows
}

pub fn write(file: &Path, load: &Load) -> Result<(), String> {
    let json = serde_json::to_string(load).map_err(|e| e.to_string())?;
    // lint: allow(asset-write) a load handed to the scene's bench process, deleted after
    std::fs::write(file, json).map_err(|e| format!("{}: {e}", file.display()))
}

pub fn read_all(dir: &Path) -> Result<Vec<Load>, String> {
    let mut loads = Vec::new();
    while let Ok(json) = std::fs::read_to_string(dir.join(format!("{}.json", loads.len()))) {
        let load = serde_json::from_str(&json).map_err(|e| format!("{}: {e}", dir.display()))?;
        loads.push(load);
    }
    Ok(loads)
}

#[derive(Clone, Default)]
pub struct StepClock(Rc<RefCell<Vec<(String, f64)>>>);

impl StepClock {
    fn push(&self, name: &str, ms: f64) {
        self.0.borrow_mut().push((name.to_string(), ms));
    }

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
    let converted = rusty_asset::convert::conversions();
    let cooked = rusty_asset::cook::cooks();
    let mut probe: Option<Box<dyn LoadProbe>> = Some(Box::new(clock.clone()));
    let path = path.to_string_lossy();
    let mut harness = Harness::on_project_scene_timed(out_dir, &path, settings, &mut probe)?;
    harness.set_size(opts.size.0, opts.size.1);
    if let Some(renderer) = harness.renderer() {
        renderer.set_switches(opts.switches);
        renderer.stream_textures(true); // as a window does: the rows measure what plays
        if opts.warm {
            renderer.warm_pipelines(false);
        }
    }
    let cameras = super::views::screen_cameras(&harness.world.borrow().scene().borrow());
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
    let converted = rusty_asset::convert::conversions().saturating_sub(converted);
    let cooked = rusty_asset::cook::cooks().saturating_sub(cooked);
    let mut counts = vec![
        ("models_parsed".to_string(), parsed as f64),
        (CONVERTED.to_string(), converted as f64),
        ("models_cooked".to_string(), cooked as f64),
    ];
    counts.extend(harness.renderer().map(render_counts).into_iter().flatten());
    let steps = clock.0.take();
    Ok((
        harness,
        Load {
            steps,
            counts,
            cameras,
        },
    ))
}

fn render_counts(r: &mut rusty_render::Renderer) -> [(String, f64); 3] {
    let (mut compressed, mut decoded) = (0, 0);
    for t in r.resident_textures() {
        *(if t.format().is_compressed() {
            &mut compressed
        } else {
            &mut decoded
        }) += 1;
    }
    [
        ("textures_compressed".into(), compressed as f64),
        ("textures_decoded".into(), decoded as f64),
        ("upload_mb".into(), mb(r.frame_counters.uploads.bytes)),
    ]
}

#[cfg(test)]
#[path = "load_cook_tests.rs"]
mod cook_tests;
#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
