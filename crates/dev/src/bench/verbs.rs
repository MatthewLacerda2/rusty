//! The script rows of an armed run (`make bench-scenes SCRIPT=1`, #2356): where
//! `cpu.update_scripts` goes, by engine verb (`stats::verbs`).
//!
//! Each measured frame adds `script.<Namespace.Verb>` (the verb's exclusive ms
//! at the reference CPU clock) and `script.<Namespace.Verb>.calls` for every verb
//! called so far in the window, zero on a frame it was not, so each is averaged
//! over the same frames as `cpu.update_scripts`; `script.calls`, every verb's
//! calls; `script.overhead`, the wrappers' own time inside `cpu.update_scripts`;
//! and `script.lua`, what of `cpu.update_scripts` neither a verb nor the
//! wrappers hold: the VM running the scripts, and the calls the wrappers do not
//! see (a handle's methods). The verb rows count every system's calls (`LateUpdate`, timers,
//! callbacks), so they and `script.lua` need not sum to `cpu.update_scripts`.
//! [`keep_top`] then keeps a scene's [`TOP`] costliest verbs and sums the rest.

use rusty_core::frame_stats::FrameStats;

use super::scenes::report::Row;
use super::Samples;
use crate::stats::verbs::{self, UPDATE_SYSTEM};

/// Whether `metric` is a script row, which a viewpoint or run without it holds
/// at zero: a verb it never called (`scenes::fold`).
pub fn absent_is_zero(metric: &str) -> bool {
    metric.starts_with("script.")
}

/// The verbs a scene's report lists by name; the rest are `script.other_verbs`.
pub const TOP: usize = 10;

/// The verbs a measured window has seen, and how many frames it has pushed.
#[derive(Default)]
pub(super) struct Window {
    seen: Vec<String>,
    frames: usize,
}

impl Window {
    /// Drop what the verbs did before the window: warm-up frames, the load.
    pub(super) fn start() -> Self {
        verbs::take();
        Self::default()
    }

    /// This frame's script rows, each time scaled by the frame's CPU clock
    /// `speed`. Nothing when the run is not armed; a frame the probe missed
    /// pushes nothing, as it pushes no `cpu.` rows.
    pub(super) fn sample(&mut self, samples: &mut Samples, stats: &FrameStats, speed: Option<f64>) {
        let frame = verbs::take();
        let Some(speed) = speed.filter(|_| verbs::armed()) else {
            return;
        };
        let update = stats.tick_systems.iter().find(|(n, _)| *n == UPDATE_SYSTEM);
        self.push(samples, &frame, update.map_or(0.0, |(_, ms)| *ms), speed);
    }

    /// [`Self::sample`]'s rows for `frame`, whose `cpu.update_scripts` took
    /// `update_ms` as measured.
    fn push(&mut self, samples: &mut Samples, frame: &verbs::Frame, update_ms: f64, speed: f64) {
        for verb in &frame.verbs {
            if !self.seen.contains(&verb.name) {
                for _ in 0..self.frames {
                    samples.push(&format!("script.{}", verb.name), 0.0);
                    samples.push(&format!("script.{}.calls", verb.name), 0.0);
                }
                self.seen.push(verb.name.clone());
            }
        }
        for name in &self.seen {
            let verb = frame.verbs.iter().find(|v| &v.name == name);
            let (ms, calls) = verb.map_or((0.0, 0), |v| (v.ms, v.calls));
            samples.push(&format!("script.{name}"), ms * speed);
            samples.push(&format!("script.{name}.calls"), f64::from(calls));
        }
        let calls: u32 = frame.verbs.iter().map(|v| v.calls).sum();
        samples.push("script.calls", f64::from(calls));
        let lua = (update_ms - frame.update_ms - frame.overhead_ms).max(0.0);
        samples.push("script.lua", lua * speed);
        samples.push("script.overhead", frame.overhead_ms * speed);
        self.frames += 1;
    }
}

/// Keep the [`TOP`] verbs by average time, costliest first, each beside its
/// `.calls` row, after `script.calls`, `script.lua` and `script.overhead`; sum the rest into
/// `script.other_verbs` and `script.other_verbs.calls` (their p95 and spread the
/// sums of the verbs', an upper bound). The other rows keep their order.
pub fn keep_top(rows: &mut Vec<Row>) {
    let is_script = |r: &Row| r.metric.starts_with("script.");
    let (script, other): (Vec<Row>, Vec<Row>) = rows.drain(..).partition(is_script);
    *rows = other;
    let named = |m: &str| ["script.calls", "script.lua", "script.overhead"].contains(&m);
    rows.extend(script.iter().filter(|r| named(&r.metric)).cloned());
    let calls_of = |verb: &Row| {
        let metric = format!("{}.calls", verb.metric);
        script.iter().find(|r| r.metric == metric).cloned()
    };
    let mut verbs: Vec<&Row> = script
        .iter()
        .filter(|r| !named(&r.metric) && !r.metric.ends_with(".calls"))
        .collect();
    verbs.sort_by(|a, b| b.avg.total_cmp(&a.avg));
    let (mut rest, mut rest_calls) = (sum_row("script.other_verbs"), None);
    for (n, verb) in verbs.into_iter().enumerate() {
        if n < TOP {
            rows.push(verb.clone());
            rows.extend(calls_of(verb));
            continue;
        }
        add(&mut rest, verb);
        let total = rest_calls.get_or_insert_with(|| sum_row("script.other_verbs.calls"));
        if let Some(calls) = calls_of(verb) {
            add(total, &calls);
        }
    }
    if let Some(calls) = rest_calls {
        rows.push(rest);
        rows.push(calls);
    }
}

fn sum_row(metric: &str) -> Row {
    Row::value(metric.into(), 0.0)
}

fn add(sum: &mut Row, row: &Row) {
    sum.avg += row.avg;
    sum.p95 += row.p95;
    sum.lo += row.lo;
    sum.hi += row.hi;
}

#[cfg(test)]
#[path = "verbs_tests.rs"]
mod tests;
