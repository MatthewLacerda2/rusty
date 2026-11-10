//! A headless bake's progress as log lines (#1667), Unity's batch-mode bake log.
//!
//! `Lighting.Generate()`, `Lighting.Bake()`, `Lighting.BakeLightmaps()`,
//! `Probe.Bake()` and `Reflection.Bake()` block the script (and the `session-mcp`
//! session) that calls them, so a getter could never be polled mid-bake. Instead the
//! bake hands a [`BakeLog`] where it is ([`Progress`]); the log prints the steps it
//! will run when its caller names them ([`BakeLog::plan`], #1897), then a line at
//! each step change, every [`INTERVAL`] in between and for each lightmap page
//! encoded. Each line carries the time since the bake began (#1891), an ETA from the
//! step's rate over its last 2 minutes (`rate::WINDOW`, #1890), and the steps still to come.
//! At the end it prints what each step took, the slowest lightmap meshes and the
//! total (`summary`, #1891). Every second of the bake belongs to one step's line, so
//! they add up to the total: work between two steps that is neither (writing the
//! lightmaps, placing probes, saving) is a step of its own ([`BakeLog::enter`]):
//!
//! ```text
//! [Lighting] steps: lightmaps → lightmap pages → light probes → reflection probes → occlusion
//! [Lighting] 26:02 lightmaps 41% (612/1501 meshes, 3.1M/7.6M texels), ~38 min left (then: lightmap pages, light probes, reflection probes, occlusion)
//! [Lighting] 1:05:10 lightmap pages 50% (6/12 pages encoded), ~30 s left (then: light probes, reflection probes, occlusion)
//! [Lighting] 1:07:43 light probes, bounce 2 of at most 5: 40% (48/120 probes), ~1 min left in this bounce (then: reflection probes, occlusion)
//! [Lighting] 1:09:01 occlusion 62% (2260/3645 cells), ~4 s left
//! [Lighting] lightmaps took 1 h 4 min (7.6M texels)
//! [Lighting] slowest lightmap meshes (worker time over every thread):
//! [Lighting]   Hall_Ceiling: 210.4k texels, 312.5 s, 673 texels/s
//! [Lighting] done in 1 h 9 min
//! ```
//!
//! Platform-side on purpose: the clock is read here, never in the sim. The lightmap
//! bake only counts into its atomic `BakeProgress` (and, when logged, says where each
//! batch starts and ends to the [`MeshClocks`] this module lends it, which times
//! them); the probe
//! bakes step on the caller's thread. Each [`Line`] says whether it is a milestone
//! (the lightmap step's start or the total; a probe step's start is already in the
//! console as the job's own `Baking …` line), which the script verbs keep in the
//! console as well; every line goes to stderr, the one stream a `play` run shows
//! while the call blocks (`session-mcp`'s stdout carries only JSON-RPC), and to
//! the thread's [`listen`]er, which turns it into MCP progress (#1999).

use std::cell::RefCell;
use std::time::{Duration, Instant};

use crate::scripting::ConsoleLogs;

mod format;
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

pub(in crate::dev::bake) use format::{duration, eta};
pub use mesh_clock::{MeshClocks, MeshTime};
pub use step::{stage_progress, Step};
pub use summary::MeshCost;

/// The longest a running step goes without a line.
pub const INTERVAL: Duration = Duration::from_secs(5);

/// Where a bake is: its step, the work done of the step's total (texels or probes),
/// and for lightmaps the meshes finished of theirs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress {
    pub step: Step,
    pub done: u64,
    pub total: u64,
    pub meshes: Option<(u64, u64)>,
}

/// One line of the log; a milestone, which the console keeps too, is the lightmap
/// step's start or the total time.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub text: String,
    pub milestone: bool,
}

/// The step running: its rate's recent samples and the work done at the last look.
struct Since {
    step: Step,
    rate: rate::Window,
    done: u64,
}

type Clock<'a> = Box<dyn FnMut() -> Duration + 'a>;
type Sink<'a> = Box<dyn FnMut(Line) + 'a>;

/// Turns a bake's [`Progress`] into [`Line`]s, at most one every [`INTERVAL`] within
/// a step. An off log ([`BakeLog::off`]) reads no clock and prints nothing.
pub struct BakeLog<'a> {
    on: Option<(Clock<'a>, Sink<'a>)>,
    started: Option<Duration>,
    last_line: Duration,
    since: Option<Since>,
    plan: Vec<Step>,
    steps: Vec<summary::StepRun>,
    meshes: Vec<MeshCost>,
    /// The lightmaps OIDN cleaned and its worker time (#1884).
    denoised: Option<(u64, Duration)>,
}

impl<'a> BakeLog<'a> {
    /// A log reading `clock` (time since any fixed start) and writing to `out`.
    pub fn new(clock: impl FnMut() -> Duration + 'a, out: impl FnMut(Line) + 'a) -> Self {
        Self {
            on: Some((Box::new(clock), Box::new(out))),
            ..Self::off()
        }
    }

    /// The log a script verb uses: the wall clock, every line to stderr and to this
    /// thread's [`listen`]er (#1999), and the milestones to `console` too.
    pub fn to_console(console: &'a RefCell<ConsoleLogs>) -> Self {
        let start = Instant::now();
        Self::new(
            move || start.elapsed(),
            move |line: Line| {
                eprintln!("{}", line.text);
                listen::tell(&line.text); // an MCP call waiting on the verb (#1999)
                if line.milestone {
                    console.borrow_mut().info(line.text);
                }
            },
        )
    }

    /// No log: tests and callers nobody watches.
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
        }
    }

    pub fn is_on(&self) -> bool {
        self.on.is_some()
    }

    /// Say the steps this bake will run, once, as it begins (#1897); each line then
    /// names the ones still to come. A step with nothing to do is skipped silently.
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

    /// The lightmap meshes' costs, for the end's slowest-meshes lines (#1891).
    pub fn mesh_costs(&mut self, meshes: Vec<MeshCost>) {
        if self.is_on() {
            self.meshes = meshes;
        }
    }

    /// The lightmaps OIDN cleaned and the time it took (#1884), for the end's
    /// denoise line. It runs on the trace's workers, chart by chart, so it is no
    /// step of its own: its time is inside the lightmaps step's.
    pub fn denoised(&mut self, charts: u64, busy: Duration) {
        if self.is_on() {
            self.denoised = Some((charts, busy));
        }
    }

    /// Begin `step`, work between two reported steps that is neither (#1891): it
    /// prints no line of its own, only what it took at the end. The next
    /// [`observe`](Self::observe) begins the step after it.
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

    /// Note where the bake is: a line when the step changed or [`INTERVAL`] passed.
    pub fn observe(&mut self, p: Progress) {
        let Some((clock, out)) = self.on.as_mut() else {
            return;
        };
        let now = clock();
        let started = *self.started.get_or_insert(now);
        let changed = self.since.as_ref().is_none_or(|s| s.step != p.step);
        // Encoding a page takes seconds, and each one is said as it finishes.
        let ticked =
            p.step == Step::LightmapPages && self.since.as_ref().is_some_and(|s| s.done != p.done);
        // `Lighting.Bake()` / `Generate()` already say each probe step in the console
        // (`LightingBake::step`'s own lines), so only the lightmaps' start is kept there.
        let milestone = changed && p.step == Step::Lightmaps;
        if changed {
            // The first step owns the time since the bake began, so none is lost.
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
        // The rate starts when the total is known, so rasterizing never skews it.
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

    /// What each step took, the slowest lightmap meshes, and the total time since
    /// the bake began (its [`plan`](Self::plan), or else its first step) as a
    /// milestone; nothing when nothing began.
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
        out(Line {
            text: format!("[Lighting] done in {}", format::duration(took)),
            milestone: true,
        });
        self.since = None;
        self.plan.clear();
        self.steps.clear();
        self.meshes.clear();
    }
}
