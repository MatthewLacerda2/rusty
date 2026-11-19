//! crates/dev/src/stats/mod.rs — frame-stats collection (#433): the clock side of `FrameStats`.
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
//! time is the sum of its systems' times and its collider-AABB flush (`flush_bounds`).
//! `frame_ms` is the [`GameWorld::tick`](rusty_app::GameWorld::tick), from its
//! `begin_tick` (after the play transition) to its `end_tick`, so it holds what no
//! system does (#2342). `end_tick` is the one place a frame closes: its system rows,
//! each name summed over the tick (`flush_bounds` runs once a stage), `frame_ms` and
//! the frame count; a schedule frame run outside a host's tick is never closed.
//!
//! **Sessions.** The stats restart on each Play session's first frame, so a window
//! reports the current run; a frame drawn outside Play reaches only the recent ring
//! ([`record_window`]), so after Stop the session holds until the next Play.
//!
//! Render counters come from the renderer when it draws (a harness screenshot, a
//! window frame, #2205; [`record_render_into`]). [`verbs`] splits script time by verb.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use super::capture::RenderedFrame;
use rusty_app::{GameWorld, Resources, Stage, SystemProbe, World};
use rusty_core::frame_stats::{DrawnFrame, FrameStats, Series};

mod counters;
mod load;
pub mod verbs;
mod window;
pub use load::LoadTimer;
pub use window::{known_keys, record_window, whole_count, window_ended, window_opt, WindowFrame};

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
    /// This tick's per-system (and per-part) times, held until `end_tick`, after
    /// `end_frame` knows whether the frame opened a new Play session (and so must
    /// reset the stats first).
    system_ms: Vec<(&'static str, f64)>,
    /// Each change channel's `logged` count at the last frame's end, for its delta.
    logged: BTreeMap<&'static str, u64>,
    /// The [`LoadTimer`]'s samples of a load this frame, recorded at its end.
    loads: LoadSamples,
    /// When the host's tick began (`begin_tick`), for `frame_ms` at its end.
    tick_start: Option<Instant>,
    /// This tick ran a schedule frame (`end_frame`), so `end_tick` closes one: an
    /// edit tick records nothing.
    frame_ran: bool,
}

/// A finished load's `load_*` samples (#2123), held until the frame's end: a load in
/// a Play session's first tick runs before `end_frame` starts the stats afresh, and
/// recorded at once its samples would be wiped with the rest.
pub type LoadSamples = Rc<RefCell<Vec<(String, f64)>>>;

impl TimingProbe {
    pub fn new(stats: Rc<RefCell<FrameStats>>) -> Self {
        Self {
            stats,
            stage_ms: [0.0; 4],
            system_ms: Vec::new(),
            logged: BTreeMap::new(),
            loads: LoadSamples::default(),
            tick_start: None,
            frame_ran: false,
        }
    }

    /// The buffer a [`LoadTimer`] fills, recorded into the stats at each frame's end.
    pub fn loads(&self) -> LoadSamples {
        Rc::clone(&self.loads)
    }
}

impl SystemProbe for TimingProbe {
    fn run(&mut self, stage: Stage, name: &'static str, system: &mut dyn FnMut()) {
        verbs::enter_system(name);
        let ms = time_ms(system);
        if let Some(slot) = Stage::FRAME_ORDER.iter().position(|s| *s == stage) {
            self.stage_ms[slot] += ms;
        }
        self.system_ms.push((name, ms));
    }

    fn run_part(&mut self, name: &'static str, part: &mut dyn FnMut()) {
        verbs::enter_system(name);
        let ms = time_ms(part);
        self.system_ms.push((name, ms));
    }

    fn end_frame(&mut self, world: &World, res: &Resources) {
        let mut stats = self.stats.borrow_mut();
        // The first frame of a Play session (`advance_frame` has just made it 1)
        // starts the stats afresh.
        if res.play_frame() == 1 {
            *stats = FrameStats::default();
        }
        for (key, ms) in self.loads.borrow_mut().drain(..) {
            stats.record(&key, ms);
        }
        for (stage, ms) in Stage::FRAME_ORDER.iter().zip(self.stage_ms) {
            stats.record(stage_key(*stage), ms);
        }
        self.stage_ms = [0.0; 4];
        counters::record_world(&mut stats, world, res);
        let changes = world.scene.borrow().world.change_stats();
        let changes = changes.into_iter().map(|s| (s.name, s.logged));
        counters::record_changes(&mut stats, changes, &mut self.logged);
        self.frame_ran = true;
    }

    fn begin_tick(&mut self) {
        self.system_ms.clear();
        self.tick_start = Some(Instant::now());
    }

    /// Closes the frame, if a schedule frame ran this tick: every system row (one
    /// sample a name, summed over the stages it ran in), `frame_ms`, the count.
    fn end_tick(&mut self) {
        let start = self.tick_start.take();
        let ran = std::mem::take(&mut self.frame_ran);
        let systems = summed(self.system_ms.drain(..));
        let Some(start) = start.filter(|_| ran) else {
            return; // an edit tick: no frame stats
        };
        let mut stats = self.stats.borrow_mut();
        for (name, ms) in systems {
            stats.record_system(name, ms);
        }
        stats.record("frame_ms", start.elapsed().as_secs_f64() * 1000.0);
        stats.end_tick();
    }
}

/// Run `f`, returning how long it took in milliseconds.
fn time_ms(f: &mut dyn FnMut()) -> f64 {
    let start = Instant::now();
    f();
    start.elapsed().as_secs_f64() * 1000.0
}

/// Each name's times added up, in the order the names first ran.
fn summed(times: impl Iterator<Item = (&'static str, f64)>) -> Vec<(&'static str, f64)> {
    let mut out: Vec<(&'static str, f64)> = Vec::new();
    for (name, ms) in times {
        match out.iter_mut().find(|(n, _)| *n == name) {
            Some((_, sum)) => *sum += ms,
            None => out.push((name, ms)),
        }
    }
    out
}

/// Install a [`TimingProbe`] and a [`LoadTimer`] on `world`, writing into the script
/// runtime's stats cell so `Debug.Stats()` reads what they record. Returns that cell.
pub fn install(world: &mut GameWorld) -> Rc<RefCell<FrameStats>> {
    let stats = world.script_manager().stats_cell();
    let timing = TimingProbe::new(Rc::clone(&stats));
    world.set_load_probe(Some(Box::new(LoadTimer::new(timing.loads()))));
    world.set_probe(Some(Box::new(timing)));
    stats
}

/// Fold one rendered frame into `stats` as a frame of the session (a harness
/// render): [`record_render_into`] it.
pub fn record_render(stats: &mut FrameStats, frame: &RenderedFrame) {
    stats.record_frame(true, |drawn| record_render_into(drawn, frame));
}

/// Fold one rendered frame (`drawn_frames`) into the frame being recorded: its
/// counters, its CPU recording ms, and where the adapter has timestamps its GPU ms
/// in total (`gpu_ms`), per pass kind (#835), untimed (`gpu_other_ms`) and waiting
/// for the CPU (`gpu_between_submits_ms`).
pub fn record_render_into(drawn: &mut DrawnFrame<'_>, frame: &RenderedFrame) {
    for (key, value) in frame.counters.pairs() {
        drawn.record(key, value as f64);
    }
    drawn.record("renderer_ms", frame.cpu_ms);
    drawn.rendered();
    if let Some(gpu) = &frame.gpu {
        drawn.record("gpu_ms", gpu.total_ms());
        drawn.record("gpu_other_ms", gpu.other_ms);
        drawn.record("gpu_between_submits_ms", gpu.between_submits_ms);
        for (pass, ms) in &gpu.passes {
            drawn.record_gpu_pass(pass.name(), *ms);
        }
    }
}

/// The stats as the Lua table `Debug.Stats()` / `Harness.Stats()` return:
/// `{ frames = n, drawn_frames = n, <metric> = {last, min, avg, max, samples}, …,
/// systems = { <name> = … }, gpu_passes = { <pass> = … } }` — `gpu_passes` only
/// where GPU time was measured.
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
    out.set("drawn_frames", stats.drawn_frames)?;
    for (key, s) in &stats.metrics {
        out.set(key.as_str(), series(s)?)?;
    }
    let systems = lua.create_table()?;
    for (name, s) in &stats.systems {
        systems.set(name.as_str(), series(s)?)?;
    }
    out.set("systems", systems)?;
    // Absent, not empty, where the adapter has no timestamps (#835).
    if !stats.gpu_passes.is_empty() {
        let passes = lua.create_table()?;
        for (name, s) in &stats.gpu_passes {
            passes.set(name.as_str(), series(s)?)?;
        }
        out.set("gpu_passes", passes)?;
    }
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
#[path = "load_tests.rs"]
mod stats_load_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod stats_tests;
#[cfg(test)]
#[path = "tick_tests.rs"]
mod stats_tick_tests;
#[cfg(test)]
#[path = "wall_tests.rs"]
mod stats_wall_tests;
