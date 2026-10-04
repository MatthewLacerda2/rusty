//! src/dev/stats.rs — frame-stats collection (#433): the clock side of `FrameStats`.
//!
//! The sim may not read a clock, so it doesn't: `app::Schedule` hands every system to
//! an installed [`SystemProbe`], and [`TimingProbe`] — here, in the dev layer, which
//! the determinism rule exempts — times the call. At the end of each frame it folds
//! the stage totals and the world counters into the shared [`FrameStats`] cell that
//! `Debug.Stats()` reads. Nothing a system can see changes: the probe runs each system
//! exactly once and only ever *reads* the world.
//!
//! **Granularity.** Per-system *and* per-stage. The schedule is ~a dozen systems, so a
//! clock read around each costs well under a microsecond a frame, and "FixedUpdate is
//! slow" is only actionable once it names `update_scripts` or `step_physics`. A stage's
//! time is the sum of its systems' times.
//!
//! **Sessions.** The stats restart on the first frame of every Play session, so a
//! windowed editor reports the current run, not every run since launch.
//!
//! Render counters come from the renderer when it draws (a harness screenshot);
//! [`record_render`] folds them in. Headless runs without a screenshot report timings
//! and world counters only.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::app::{GameWorld, Resources, Stage, SystemProbe, World};
use crate::core::frame_stats::{FrameStats, Series};
use crate::render::RenderCounters;

/// The metric each per-frame stage's CPU time is recorded under.
fn stage_key(stage: Stage) -> &'static str {
    match stage {
        Stage::Startup => "startup_ms",
        Stage::FixedUpdate => "fixed_update_ms",
        Stage::Update => "update_ms",
        Stage::LateUpdate => "late_update_ms",
        Stage::Render => "render_stage_ms",
    }
}

/// Times every system the schedule runs and records a frame's worth of stats.
pub struct TimingProbe {
    stats: Rc<RefCell<FrameStats>>,
    /// This frame's CPU ms per stage, indexed by [`Stage::FRAME_ORDER`] position.
    stage_ms: [f64; 4],
    /// This frame's per-system times, held until `end_frame` knows whether the frame
    /// opened a new Play session (and so must reset the stats first).
    system_ms: Vec<(&'static str, f64)>,
}

impl TimingProbe {
    pub fn new(stats: Rc<RefCell<FrameStats>>) -> Self {
        Self {
            stats,
            stage_ms: [0.0; 4],
            system_ms: Vec::new(),
        }
    }
}

impl SystemProbe for TimingProbe {
    fn run(&mut self, stage: Stage, name: &'static str, system: &mut dyn FnMut()) {
        let start = Instant::now();
        system();
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        if let Some(slot) = Stage::FRAME_ORDER.iter().position(|s| *s == stage) {
            self.stage_ms[slot] += ms;
        }
        self.system_ms.push((name, ms));
    }

    fn end_frame(&mut self, world: &World, res: &Resources) {
        let mut stats = self.stats.borrow_mut();
        // The first frame of a Play session (`advance_frame` has just made it 1)
        // starts the stats afresh.
        if res.play_frame() == 1 {
            *stats = FrameStats::default();
        }
        for (name, ms) in self.system_ms.drain(..) {
            stats.record_system(name, ms);
        }
        let mut total = 0.0;
        for (stage, ms) in Stage::FRAME_ORDER.iter().zip(self.stage_ms) {
            stats.record(stage_key(*stage), ms);
            total += ms;
        }
        stats.record("frame_ms", total);
        self.stage_ms = [0.0; 4];
        record_world(&mut stats, world, res);
        stats.frames += 1;
    }
}

/// The world counters: entities, rigid bodies, nav agents, live particles, scripts.
fn record_world(stats: &mut FrameStats, world: &World, res: &Resources) {
    let scene = world.scene.borrow();
    let w = &scene.world;
    let particles: usize = w
        .ids_with_particles()
        .into_iter()
        .filter_map(|id| w.particles(id).map(|p| p.live_count()))
        .sum();
    stats.record("entities", w.len() as f64);
    stats.record("rigid_bodies", w.ids_with_rigidbody().len() as f64);
    stats.record("nav_agents", w.ids_with_nav_agent().len() as f64);
    stats.record("particles", particles as f64);
    stats.record("scripts", res.script_manager.live_script_count() as f64);
}

/// Install a [`TimingProbe`] on `world`, writing into the script runtime's stats cell
/// so `Debug.Stats()` reads what the probe records. Returns that cell.
pub fn install(world: &mut GameWorld) -> Rc<RefCell<FrameStats>> {
    let stats = world.script_manager().stats_cell();
    world.set_probe(Some(Box::new(TimingProbe::new(Rc::clone(&stats)))));
    stats
}

/// Fold one rendered frame's counters, and the CPU ms the renderer spent recording
/// it, into `stats`.
pub fn record_render(stats: &mut FrameStats, counters: &RenderCounters, cpu_ms: f64) {
    for (key, value) in counters.pairs() {
        stats.record(key, value as f64);
    }
    stats.record("renderer_ms", cpu_ms);
}

/// The stats as the Lua table `Debug.Stats()` / `Harness.Stats()` return:
/// `{ frames = n, <metric> = {last, min, avg, max, samples}, …, systems = { <name> = … } }`.
pub fn to_lua(lua: &mlua::Lua, stats: &FrameStats) -> mlua::Result<mlua::Table> {
    let series = |s: &Series| -> mlua::Result<mlua::Table> {
        let t = lua.create_table()?;
        t.set("last", s.last)?;
        t.set("min", s.min)?;
        t.set("avg", s.avg)?;
        t.set("max", s.max)?;
        t.set("samples", s.samples)?;
        Ok(t)
    };
    let out = lua.create_table()?;
    out.set("frames", stats.frames)?;
    for (key, s) in &stats.metrics {
        out.set(key.as_str(), series(s)?)?;
    }
    let systems = lua.create_table()?;
    for (name, s) in &stats.systems {
        systems.set(name.as_str(), series(s)?)?;
    }
    out.set("systems", systems)?;
    Ok(out)
}

/// A Lua budget table `{ metric = limit, … }` as `(metric, limit)` pairs, sorted by
/// name so the expectations a run records come out in a stable order.
pub fn budgets_from_lua(table: mlua::Table) -> mlua::Result<Vec<(String, f64)>> {
    let mut budgets = table
        .pairs::<String, f64>()
        .collect::<mlua::Result<Vec<_>>>()?;
    budgets.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(budgets)
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
