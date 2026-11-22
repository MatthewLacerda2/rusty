use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use super::capture::RenderedFrame;
use rusty_app::{GameWorld, Resources, Stage, SystemProbe, World};
use rusty_core::frame_stats::{DrawnFrame, FrameStats, Series};

mod clock;
mod counters;
mod load;
pub mod verbs;
mod window;
pub use clock::{ms_between, ms_since, now};
pub use load::LoadTimer;
pub use window::{known_keys, record_window, whole_count, window_ended, window_opt, WindowFrame};

fn stage_key(stage: Stage) -> &'static str {
    match stage {
        Stage::Startup => "startup_ms",
        Stage::FixedUpdate => "fixed_update_ms",
        Stage::Update => "update_ms",
        Stage::LateUpdate => "late_update_ms",
        Stage::Render => "render_stage_ms",
    }
}

pub struct TimingProbe {
    stats: Rc<RefCell<FrameStats>>,
    stage_ms: [f64; 4],
    system_ms: Vec<(&'static str, f64)>,
    part_ms: Vec<(&'static str, f64)>,
    logged: BTreeMap<&'static str, u64>,
    loads: LoadSamples,
    tick_start: Option<Instant>,
    frame_ran: bool,
    end_frame_ms: f64,
}

pub type LoadSamples = Rc<RefCell<Vec<(String, f64)>>>;

impl TimingProbe {
    pub fn new(stats: Rc<RefCell<FrameStats>>) -> Self {
        Self {
            stats,
            stage_ms: [0.0; 4],
            system_ms: Vec::new(),
            part_ms: Vec::new(),
            logged: BTreeMap::new(),
            loads: LoadSamples::default(),
            tick_start: None,
            frame_ran: false,
            end_frame_ms: 0.0,
        }
    }

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
        self.part_ms.push((name, ms));
    }

    fn end_frame(&mut self, world: &World, res: &Resources) {
        let start = verbs::now();
        self.record_frame(world, res);
        self.end_frame_ms += ms_since(start);
    }

    fn begin_tick(&mut self) {
        self.system_ms.clear();
        self.part_ms.clear();
        self.end_frame_ms = 0.0;
        self.tick_start = Some(verbs::now());
    }

    fn end_tick(&mut self) {
        let start = self.tick_start.take();
        let tick_end = verbs::now();
        let ran = std::mem::take(&mut self.frame_ran);
        let systems = summed(self.system_ms.drain(..));
        let parts = summed(self.part_ms.drain(..));
        let Some(start) = start.filter(|_| ran) else {
            return;
        };
        let mut stats = self.stats.borrow_mut();
        #[cfg(test)]
        verbs::advance(counters::END_COST.with(std::cell::Cell::get));
        for (name, ms) in systems {
            stats.record_system(name, ms);
        }
        let mut beside = 0.0;
        for (name, ms) in parts {
            stats.record(&format!("{name}_ms"), ms);
            beside += ms;
        }
        let tick = tick_end.saturating_duration_since(start).as_secs_f64() * 1000.0;
        stats.record("frame_ms", (tick - beside - self.end_frame_ms).max(0.0));
        stats.record("probe_ms", self.end_frame_ms + ms_since(tick_end));
        stats.end_tick();
    }
}

fn time_ms(f: &mut dyn FnMut()) -> f64 {
    let start = verbs::now();
    f();
    ms_since(start)
}

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

pub fn install(world: &mut GameWorld) -> Rc<RefCell<FrameStats>> {
    let stats = world.script_manager().stats_cell();
    let timing = TimingProbe::new(Rc::clone(&stats));
    world.set_load_probe(Some(Box::new(LoadTimer::new(timing.loads()))));
    world.set_probe(Some(Box::new(timing)));
    stats
}

pub fn record_render(stats: &mut FrameStats, frame: &RenderedFrame) {
    stats.record_frame(true, |drawn| record_render_into(drawn, frame));
}

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
    if !stats.gpu_passes.is_empty() {
        let passes = lua.create_table()?;
        for (name, s) in &stats.gpu_passes {
            passes.set(name.as_str(), series(s)?)?;
        }
        out.set("gpu_passes", passes)?;
    }
    Ok(out)
}

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
#[path = "probe_cost_tests.rs"]
mod stats_probe_cost_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod stats_tests;
#[cfg(test)]
#[path = "tick_tests.rs"]
mod stats_tick_tests;
#[cfg(test)]
#[path = "wall_tests.rs"]
mod stats_wall_tests;
