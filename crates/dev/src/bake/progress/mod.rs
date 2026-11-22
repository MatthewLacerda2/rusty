use std::cell::RefCell;
use std::time::{Duration, Instant};

use rusty_script::scripting::ConsoleLogs;

pub(crate) mod format;
pub mod listen;
mod mesh_clock;
#[cfg(test)]
mod pages_tests;
#[cfg(test)]
mod plan_tests;
mod rate;
#[cfg(test)]
mod rate_tests;
#[cfg(test)]
mod stage_tests;
mod step;
mod summary;
#[cfg(test)]
mod summary_tests;
#[cfg(test)]
mod tests;

pub(in crate::bake) use format::{duration, eta};
pub use mesh_clock::{MeshClocks, MeshTime};
pub use step::{stage_progress, Step};
pub use summary::MeshCost;

pub const INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub step: Step,
    pub done: u64,
    pub total: u64,
    pub meshes: Option<(u64, u64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub milestone: bool,
}

struct Since {
    step: Step,
    rate: rate::Window,
    done: u64,
}

type Clock<'a> = Box<dyn FnMut() -> Duration + 'a>;
type Sink<'a> = Box<dyn FnMut(Line) + 'a>;

pub struct BakeLog<'a> {
    on: Option<(Clock<'a>, Sink<'a>)>,
    started: Option<Duration>,
    last_line: Duration,
    since: Option<Since>,
    plan: Vec<Step>,
    steps: Vec<summary::StepRun>,
    meshes: Vec<MeshCost>,
    denoised: Option<(u64, Duration)>,
    peak: Option<u64>,
}

impl<'a> BakeLog<'a> {
    pub fn new(clock: impl FnMut() -> Duration + 'a, out: impl FnMut(Line) + 'a) -> Self {
        Self {
            on: Some((Box::new(clock), Box::new(out))),
            ..Self::off()
        }
    }

    pub fn to_console(console: &'a RefCell<ConsoleLogs>) -> Self {
        let start = Instant::now();
        Self::new(
            move || start.elapsed(),
            move |line: Line| {
                eprintln!("{}", line.text);
                listen::tell(&line.text);
                if line.milestone {
                    console.borrow_mut().info(line.text);
                }
            },
        )
    }

    pub fn off() -> Self {
        Self {
            on: None,
            started: None,
            last_line: Duration::ZERO,
            since: None,
            plan: Vec::new(),
            steps: Vec::new(),
            meshes: Vec::new(),
            denoised: None,
            peak: None,
        }
    }

    pub fn is_on(&self) -> bool {
        self.on.is_some()
    }

    pub fn plan(&mut self, steps: &[Step]) {
        let Some((clock, out)) = self.on.as_mut() else {
            return;
        };
        self.started.get_or_insert(clock());
        self.plan = steps.to_vec();
        let names: Vec<&str> = steps.iter().map(|s| s.name()).collect();
        out(Line {
            text: format!("[Lighting] steps: {}", names.join(" → ")),
            milestone: false,
        });
    }

    pub fn mesh_costs(&mut self, meshes: Vec<MeshCost>) {
        if self.is_on() {
            self.meshes = meshes;
        }
    }

    pub fn peak_memory(&mut self, bytes: u64) {
        if self.is_on() {
            self.peak = Some(self.peak.map_or(bytes, |p| p.max(bytes)));
        }
    }

    pub fn denoised(&mut self, charts: u64, busy: Duration) {
        if self.is_on() {
            self.denoised = Some((charts, busy));
        }
    }

    pub fn enter(&mut self, step: Step) {
        let Some((clock, _)) = self.on.as_mut() else {
            return;
        };
        let now = clock();
        let started = *self.started.get_or_insert(now);
        let from = if self.steps.is_empty() { started } else { now };
        self.steps.push(summary::StepRun::new(step, from));
        self.since = None;
    }

    pub fn observe(&mut self, p: Progress) {
        let Some((clock, out)) = self.on.as_mut() else {
            return;
        };
        let now = clock();
        let started = *self.started.get_or_insert(now);
        let changed = self.since.as_ref().is_none_or(|s| s.step != p.step);
        let ticked =
            p.step == Step::LightmapPages && self.since.as_ref().is_some_and(|s| s.done != p.done);
        let milestone = changed && p.step == Step::Lightmaps;
        if changed {
            let from = if self.steps.is_empty() { started } else { now };
            self.steps.push(summary::StepRun::new(p.step, from));
            let rate = rate::Window::default();
            self.since = Some(Since {
                step: p.step,
                rate,
                done: p.done,
            });
        }
        let (Some(since), Some(run)) = (self.since.as_mut(), self.steps.last_mut()) else {
            return;
        };
        since.done = p.done;
        run.total = p.total;
        if p.total > 0 {
            since.rate.note(now, p.done);
        }
        if !changed && !ticked && now.saturating_sub(self.last_line) < INTERVAL {
            return;
        }
        self.last_line = now;
        let left = since
            .rate
            .from()
            .and_then(|from| format::eta(from, now, &p));
        let then = format::then(&self.plan, p.step);
        let elapsed = now.saturating_sub(started);
        out(Line {
            text: format::progress(&p, elapsed, left, &then),
            milestone,
        });
    }

    pub fn finish(&mut self) {
        let Some((clock, out)) = self.on.as_mut() else {
            return;
        };
        let Some(started) = self.started.take() else {
            return;
        };
        let now = clock();
        let denoised = self.denoised.take();
        for text in summary::lines(&self.steps, now, &mut self.meshes, denoised) {
            out(Line {
                text,
                milestone: false,
            });
        }
        let took = now.saturating_sub(started);
        let peak = (self.peak.take()).map(|b| format!(", process peak {}", format::gb(b)));
        out(Line {
            text: format!(
                "[Lighting] done in {}{}",
                format::duration(took),
                peak.unwrap_or_default()
            ),
            milestone: true,
        });
        self.since = None;
        self.plan.clear();
        self.steps.clear();
        self.meshes.clear();
    }
}
