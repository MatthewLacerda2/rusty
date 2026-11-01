//! A headless bake's progress as log lines (#1667), Unity's batch-mode bake log.
//!
//! `Lighting.Generate()`, `Lighting.Bake()`, `Lighting.BakeLightmaps()`,
//! `Probe.Bake()` and `Reflection.Bake()` block the script (and the `session-mcp`
//! session) that calls them, so a getter could never be polled mid-bake. Instead the
//! bake hands a [`BakeLog`] where it is ([`Progress`]); the log prints a line at each
//! step change and every [`INTERVAL`] in between, with an ETA from the step's own
//! rate, and the total time at the end:
//!
//! ```text
//! [Lighting] lightmaps 41% (612/1501 meshes, 3.1M/7.6M texels), ~38 min left
//! [Lighting] light probes, bounce 2 of at most 5: 40% (48/120 probes), ~1 min left in this bounce
//! [Lighting] reflection probes 19% (3/16 probes), ~4 min left
//! [Lighting] done in 1 h 4 min
//! ```
//!
//! Platform-side on purpose: the clock is read here, never in the sim. The lightmap
//! bake only counts into its atomic `BakeProgress`; the probe bakes step on the
//! caller's thread. Each [`Line`] says whether it is a milestone (the lightmap step's
//! start or the total; a probe step's start is already in the console as the job's own
//! `Baking …` line), which the script verbs keep in the console as well; every line goes to
//! stderr, the one stream a `play` run or an MCP client shows while the call blocks
//! (`session-mcp`'s stdout carries only JSON-RPC).

use std::cell::RefCell;
use std::time::{Duration, Instant};

use super::steps::Stage;
use crate::render::ibl::probe::bounce::MAX_BOUNCES;
use crate::scripting::ConsoleLogs;

mod format;
#[cfg(test)]
mod stage_tests;
#[cfg(test)]
mod tests;

/// The longest a running step goes without a line.
pub const INTERVAL: Duration = Duration::from_secs(5);

/// Which part of a bake is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Lightmaps,
    /// One pass of the light-probe solve, which stops at `max` or when it converges.
    LightProbes {
        bounce: u32,
        max: u32,
    },
    ReflectionProbes,
}

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

/// Where a step's rate is measured from: the first moment its total was known.
struct Since {
    step: Step,
    rate_from: Option<(Duration, u64)>,
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
}

impl<'a> BakeLog<'a> {
    /// A log reading `clock` (time since any fixed start) and writing to `out`.
    pub fn new(clock: impl FnMut() -> Duration + 'a, out: impl FnMut(Line) + 'a) -> Self {
        Self {
            on: Some((Box::new(clock), Box::new(out))),
            started: None,
            last_line: Duration::ZERO,
            since: None,
        }
    }

    /// The log a script verb uses: the wall clock, every line to stderr and the
    /// milestones to `console` too.
    pub fn to_console(console: &'a RefCell<ConsoleLogs>) -> Self {
        let start = Instant::now();
        Self::new(
            move || start.elapsed(),
            move |line: Line| {
                eprintln!("{}", line.text);
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
        }
    }

    pub fn is_on(&self) -> bool {
        self.on.is_some()
    }

    /// Note where the bake is: a line when the step changed or [`INTERVAL`] passed.
    pub fn observe(&mut self, p: Progress) {
        let Some((clock, out)) = self.on.as_mut() else {
            return;
        };
        let now = clock();
        self.started.get_or_insert(now);
        let changed = self.since.as_ref().is_none_or(|s| s.step != p.step);
        // `Lighting.Bake()` / `Generate()` already say each probe step in the console
        // (`LightingBake::step`'s own lines), so only the lightmaps' start is kept there.
        let milestone = changed && p.step == Step::Lightmaps;
        if changed {
            self.since = Some(Since {
                step: p.step,
                rate_from: None,
            });
        }
        let Some(since) = self.since.as_mut() else {
            return;
        };
        if p.total > 0 && since.rate_from.is_none() {
            since.rate_from = Some((now, p.done));
        }
        if !changed && now.saturating_sub(self.last_line) < INTERVAL {
            return;
        }
        self.last_line = now;
        let left = since.rate_from.and_then(|from| format::eta(from, now, &p));
        out(Line {
            text: format::progress(&p, left),
            milestone,
        });
    }

    /// The total time since the first [`observe`](Self::observe), as a milestone;
    /// nothing when nothing was observed.
    pub fn finish(&mut self) {
        let Some((clock, out)) = self.on.as_mut() else {
            return;
        };
        let Some(started) = self.started.take() else {
            return;
        };
        let took = clock().saturating_sub(started);
        out(Line {
            text: format!("[Lighting] done in {}", format::duration(took)),
            milestone: true,
        });
        self.since = None;
    }
}

/// Where a stepped probe bake is, as the log reads it; `None` once it is done, and for
/// a probe step with nothing to bake (no probes of its kind), which has nothing to say.
pub fn stage_progress(stage: Stage) -> Option<Progress> {
    let (step, done, total) = match stage {
        Stage::Light {
            bounce,
            done,
            total,
        } => (
            Step::LightProbes {
                bounce,
                max: MAX_BOUNCES,
            },
            done,
            total,
        ),
        Stage::Reflection { done, total } => (Step::ReflectionProbes, done, total),
        Stage::Done => return None,
    };
    if total == 0 {
        return None;
    }
    Some(Progress {
        step,
        done: done as u64,
        total: total as u64,
        meshes: None,
    })
}
